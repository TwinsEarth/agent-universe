#!/usr/bin/env bash
# Agent Universe —— 网络分区后 CRDT 收敛回归（实机多进程）
#
# 拓扑（通过“启动时不同 bootstrap + 恢复后 live dial”真实制造分区）：
#   分区阶段：p0 单独（{1} 侧）；p1 单独，p2 经 bootstrap 连 p1（{2} 侧）。
#   分区期间：两侧分别对同一键写冲突值。
#   恢复：调用 p0 的网络 API 主动 dial p1（POST /api/v1/network/bootstrap）。
#   判定：恢复后 30s 内，三节点 CRDT 对该键收敛到相同值（LWW 确定性裁决）。
#
# 用法：scripts/regression-partition.sh
# 环境：GSN_BIN / AU_PART_DIR / KEEP=1
# 退出码：0=收敛；1=未收敛/失败。

set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
GSN_BIN="${GSN_BIN:-$REPO_ROOT/gsn-core/target/release/gsn-daemon}"
WORK="${AU_PART_DIR:-/tmp/au-part-$$}"
BEARER="part-token"
PIDS=()
mkdir -p "$WORK"

[ -x "$GSN_BIN" ] || { echo "❌ 找不到二进制 $GSN_BIN（先 cargo build --release）"; exit 2; }

cleanup() {
  if [ "${KEEP:-0}" != "1" ]; then
    local p
    for p in "${PIDS[@]:-}"; do kill "$p" >/dev/null 2>&1; done
    sleep 0.5
    for p in "${PIDS[@]:-}"; do kill -9 "$p" >/dev/null 2>&1; done
  fi
}
trap cleanup EXIT

# start <name> <p2p> <api> <dir> [boot...]
start() {
  local name="$1" p2p="$2" api="$3" dir="$4"; shift 4
  mkdir -p "$dir"
  local log="$dir/node.log"
  local boot_args=() b
  for b in "$@"; do boot_args+=(--bootstrap "$b"); done
  REST_BEARER_TOKEN="$BEARER" \
    "$GSN_BIN" --listen 127.0.0.1 --port "$p2p" --api-port "$api" \
      --data-dir "$dir" --mode full "${boot_args[@]}" >"$log" 2>&1 &
  local pid=$!; PIDS+=("$pid")
  local i
  for i in $(seq 1 80); do
    curl -fsS "127.0.0.1:$api/health" >/dev/null 2>&1 && break
    kill -0 "$pid" 2>/dev/null || { echo "      ⚠️ $name 退出："; tail -5 "$log"; return 1; }
    sleep 0.5
  done
  local pidid
  pidid="$(grep -Eo 'Peer ID: [A-Za-z0-9]+' "$log" | head -1 | awk '{print $3}')"
  echo "   • $name pid=$pid p2p=$p2p api=$api PeerId=${pidid:-?}"
}

# 读某节点某键的值
get_val() {
  local api="$1"
  curl -fsS "127.0.0.1:$api/api/v1/crdt/k" 2>/dev/null | jq -r '.value // "NONE"'
}

# 写（带 Bearer）
put() {
  local api="$1" val="$2"
  curl -s -o /dev/null -X POST "127.0.0.1:$api/api/v1/crdt" \
    -H "Authorization: Bearer $BEARER" -H 'Content-Type: application/json' \
    -d "{\"key\":\"k\",\"value\":\"$val\"}"
}

# 触发快照广播
snap() {
  local api="$1"
  curl -s -o /dev/null -X POST "127.0.0.1:$api/api/v1/crdt-snapshot" \
    -H "Authorization: Bearer $BEARER"
}

echo "工作目录: $WORK"
echo "──────────── 1) 分区启动（p0 孤立；p2 连 p1）────────────"
start p0 4101 4102 "$WORK/p0" || exit 1
start p1 4111 4112 "$WORK/p1" || exit 1
start p2 4121 4122 "$WORK/p2" "/ip4/127.0.0.1/tcp/4111" || exit 1
sleep 8

echo "──────────── 2) 验证分区确实存在（p0 与 p1 不相连）────────────"
c0="$(curl -fsS 127.0.0.1:4102/peers | jq -r '.connected_peers')"
c1="$(curl -fsS 127.0.0.1:4112/peers | jq -r '.connected_peers')"
c2="$(curl -fsS 127.0.0.1:4122/peers | jq -r '.connected_peers')"
echo "   connected: p0=$c0 p1=$c1 p2=$c2"
# p1 应连 p2（≥1）；p0 应为 0（若经公共 relay 出现连接，需关注，但本地不互通）
if [ "${c1:-0}" -ge 1 ] && [ "${c2:-0}" -ge 1 ]; then
  echo "   ✅ {p1,p2} 分区已连通"
else
  echo "   ⚠️ {p1,p2} 未按预期连通（继续）"
fi

echo "──────────── 3) 分区期间两侧写冲突值 ────────────"
put 4102 "A-side"
put 4112 "B-side"
sleep 2
v0="$(get_val 4102)"; v1="$(get_val 4112)"; v2="$(get_val 4122)"
echo "   分区中：p0=$v0  p1=$v1  p2=$v2"
[ "$v0" = "A-side" ] && echo "   ✅ p0 见本方值" || echo "   ⚠️ p0 本地写异常($v0)"
[ "$v1" = "B-side" ] && echo "   ✅ p1 见本方值" || echo "   ⚠️ p1 本地写异常($v1)"

echo "──────────── 4) 恢复：p0 live dial p1 ────────────"
code="$(curl -s -o /dev/null -w '%{http_code}' -X POST 127.0.0.1:4102/api/v1/network/bootstrap \
  -H "Authorization: Bearer $BEARER" -H 'Content-Type: application/json' \
  -d '{"addr":"/ip4/127.0.0.1/tcp/4111"}')"
echo "   live bootstrap http=$code"
# 立即触发两侧快照广播，加速反熵
sleep 2
snap 4102; snap 4112; snap 4122

echo "──────────── 5) 判定 30s 内三方收敛 ────────────"
t0="$(date +%s%3N)"
converged=0
i=0
while [ $i -lt 60 ]; do
  v0="$(get_val 4102)"; v1="$(get_val 4112)"; v2="$(get_val 4122)"
  if [ "$v0" != "NONE" ] && [ "$v0" = "$v1" ] && [ "$v1" = "$v2" ]; then
    converged=1; break
  fi
  # 周期性再广播快照，保证反熵
  if [ $((i % 8)) -eq 0 ]; then snap 4102; snap 4112; fi
  sleep 0.5; i=$((i+1))
done
t1="$(date +%s%3N)"; ms=$((t1-t0))

if [ "$converged" = "1" ]; then
  echo "   ✅ 三节点在 ${ms}ms 内收敛到相同值：$v0"
  echo "🎉 网络分区 CRDT 收敛回归通过（<30s 要求）"
  exit 0
else
  echo "   ❌ 30s 内未收敛：p0=$v0 p1=$v1 p2=$v2"
  echo "⚠️ 分区收敛回归失败"
  exit 1
fi

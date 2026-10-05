#!/usr/bin/env bash
# Agent Universe —— 本地多节点实机回归（3 节点 / 5 节点）
#
# 用 release 二进制启动多个独立进程（不同 data-dir / 端口 / 独立 PeerId），验证：
#   节点发现、DHT 路由表填充、GossipSub 广播到达、CRDT 状态同步、节点重启恢复、
#   5 节点 mesh 形成与广播延迟。
#
# 用法：
#   scripts/regression-3and5.sh            # 跑 3 节点 + 5 节点
#   ONLY=3 scripts/regression-3and5.sh     # 只跑 3 节点
#   ONLY=5 scripts/regression-3and5.sh     # 只跑 5 节点
# 可选环境：
#   GSN_BIN=<gsn-daemon 路径>  AU_REG_DIR=<工作目录>  KEEP=1（结束后保留进程/数据）
#
# 退出码：0=全部通过；非0=有失败项。

set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
GSN_BIN="${GSN_BIN:-$REPO_ROOT/gsn-core/target/release/gsn-daemon}"
AU_REG_DIR="${AU_REG_DIR:-/tmp/au-reg-$$}"
BEARER="regtest-token"
NODES=()
PASS=0; FAIL=0

mkdir -p "$AU_REG_DIR"

if [ ! -x "$GSN_BIN" ]; then
  echo "❌ 找不到可执行二进制: $GSN_BIN（先跑 cargo build --release --workspace）"
  exit 2
fi

# ───────────── 通用函数 ─────────────
cleanup() {
  if [ "${KEEP:-0}" != "1" ]; then
    for n in "${NODES[@]:-}"; do
      [ -n "$n" ] && kill "$n" >/dev/null 2>&1
    done
    sleep 0.5
    for n in "${NODES[@]:-}"; do
      [ -n "$n" ] && kill -9 "$n" >/dev/null 2>&1
    done
  fi
}
trap cleanup EXIT

ok()   { echo "   ✅ $1"; PASS=$((PASS+1)); }
bad()  { echo "   ❌ $1"; FAIL=$((FAIL+1)); }

# 启动节点：start_node <name> <p2p_port> <api_port> <data_dir> [boot_api ...]
# 额外：写 CRDT 需要 Bearer，故所有节点注入 REST_BEARER_TOKEN。
# LOCAL_ONLY=0 时才允许节点探测公共 relay（默认 1=本地纯净组网，可复现、可重复）。
start_node() {
  local name="$1" p2p="$2" api="$3" dir="$4"; shift 4
  mkdir -p "$dir"
  local log="$dir/node.log"
  local boot_args=()
  local b
  for b in "$@"; do
    boot_args+=(--bootstrap "$b")
  done
  local extra_env=()
  if [ "${LOCAL_ONLY:-1}" = "1" ]; then
    extra_env=(GSN_DISABLE_PUBLIC_RELAY=1)
  fi
  env "${extra_env[@]}" REST_BEARER_TOKEN="$BEARER" \
    "$GSN_BIN" --listen 127.0.0.1 --port "$p2p" --api-port "$api" \
      --data-dir "$dir" --mode full "${boot_args[@]}" >"$log" 2>&1 &
  local pid=$!
  NODES+=("$pid")
  # 等 HTTP 就绪（最多 40s）
  local i
  for i in $(seq 1 80); do
    if curl -fsS "http://127.0.0.1:$api/health" >/dev/null 2>&1; then break; fi
    if ! kill -0 "$pid" 2>/dev/null; then
      echo "      ⚠️ $name 进程退出，见日志末尾："
      tail -5 "$log"
      return 1
    fi
    sleep 0.5
  done
  local peer_id
  peer_id="$(grep -Eo 'Peer ID: [A-Za-z0-9]+' "$log" | head -1 | awk '{print $3}')"
  echo "   • $name  pid=$pid  p2p=$p2p api=$api  PeerId=${peer_id:-?}" >&2
  echo "$peer_id"
}

# 等待条件（轮询）：wait_cond <api_port> <path> <grep_regex> <seconds>
wait_cond() {
  local api="$1" path="$2" re="$3" secs="$4" i
  for i in $(seq 1 $((secs*2))); do
    local out
    out="$(curl -fsS "http://127.0.0.1:$api$path" 2>/dev/null || true)"
    if echo "$out" | grep -qE "$re"; then echo "$out"; return 0; fi
    sleep 0.5
  done
  return 1
}

# ───────────── 3 节点回归 ─────────────
run_three() {
  echo "==================== 3 节点组网 ===================="
  local d0="$AU_REG_DIR/n0" d1="$AU_REG_DIR/n1" d2="$AU_REG_DIR/n2"

  local id0 id1 id2
  id0="$(start_node boot 4001 4002 "$d0")" || { bad "boot 节点启动失败"; return; }
  # boot 节点的可 dial multiaddr（本地回环 TCP）
  local boot_addr="/ip4/127.0.0.1/tcp/4001"
  id1="$(start_node node1 4011 4012 "$d1" "$boot_addr")" || { bad "node1 启动失败"; return; }
  id2="$(start_node node2 4021 4022 "$d2" "$boot_addr")" || { bad "node2 启动失败"; return; }

  # 给组网留时间
  sleep 8

  # 1) 节点发现：每个节点 connected_peers（boot 应≥2，worker 应≥1）
  local c0 c1 c2
  c0="$(curl -fsS 127.0.0.1:4002/peers | jq -r '.connected_peers')"
  c1="$(curl -fsS 127.0.0.1:4012/peers | jq -r '.connected_peers')"
  c2="$(curl -fsS 127.0.0.1:4022/peers | jq -r '.connected_peers')"
  echo "   connected_peers: boot=$c0 node1=$c1 node2=$c2"
  [ "${c0:-0}" -ge 2 ] && ok "boot 发现 ≥2 节点" || bad "boot 连接数不足（$c0）"
  [ "${c1:-0}" -ge 1 ] && ok "node1 发现 ≥1 节点" || bad "node1 连接数不足"
  [ "${c2:-0}" -ge 1 ] && ok "node2 发现 ≥1 节点" || bad "node2 连接数不足"

  # 2) DHT 路由表填充：/api/v1/network/info 的 routing_entries
  local r0
  r0="$(curl -fsS 127.0.0.1:4002/api/v1/network/info | jq -r '.routing_entries // 0')"
  echo "   boot routing_entries=$r0"
  [ "${r0:-0}" -ge 1 ] && ok "DHT 路由表非空（add_address 生效）" || bad "DHT 路由表为空"

  # 3) GossipSub 广播 + CRDT 同步：在 boot 写一个键，node1/node2 应在 30s 内收到
  local val="hello-$RANDOM"
  local code
  code="$(curl -s -o /dev/null -w '%{http_code}' -X POST 127.0.0.1:4002/api/v1/crdt \
    -H "Authorization: Bearer $BEARER" -H 'Content-Type: application/json' \
    -d "{\"key\":\"greet\",\"value\":\"$val\"}")"
  [ "$code" = "201" ] && ok "boot 本地写 CRDT 并广播（201）" || bad "CRDT 写失败 http=$code"

  if wait_cond 4012 /api/v1/crdt/greet "\"value\":\"$val\"" 30 >/dev/null; then
    ok "node1 经 GossipSub 收到 CRDT（且 LWW 一致）"
  else bad "node1 30s 内未收到 CRDT"; fi
  if wait_cond 4022 /api/v1/crdt/greet "\"value\":\"$val\"" 30 >/dev/null; then
    ok "node2 经 GossipSub 收到 CRDT"
  else bad "node2 30s 内未收到 CRDT"; fi

  # 4) 重启恢复：杀掉 node1，重启（同 data-dir，PeerId 应一致），仍能读到状态
  local pid1
  pid1="$(pgrep -f "$d1" | head -1)"
  [ -n "$pid1" ] && kill "$pid1" 2>/dev/null
  sleep 2
  local id1b
  id1b="$(start_node node1-restart 4011 4012 "$d1" "$boot_addr")" || { bad "node1 重启失败"; return; }
  sleep 6
  if wait_cond 4012 /api/v1/crdt/greet "\"value\":\"$val\"" 25 >/dev/null; then
    ok "node1 重启后恢复（身份持久化、状态重新同步）"
    if [ "$id1" != "?" ] && [ "$id1b" = "$id1" ]; then
      ok "node1 重启后 PeerId 一致（$id1b）"
    else
      echo "      (PeerId 比对: before=${id1} after=${id1b})"
    fi
  else bad "node1 重启后未恢复"; fi
}

# ───────────── 5 节点 mesh + 广播延迟 ─────────────
run_five() {
  echo "==================== 5 节点 mesh ===================="
  local dir="$AU_REG_DIR/mesh"
  local boot_addr="/ip4/127.0.0.1/tcp/5001"
  local id
  id="$(start_node mboot 5001 5002 "$dir/m0")" || { bad "mesh boot 启动失败"; return; }
  local i p2p api
  for i in 1 2 3 4; do
    p2p=$((5011 + 10*(i-1))); api=$((5012 + 10*(i-1)))
    id="$(start_node "m$i" "$p2p" "$api" "$dir/m$i" "$boot_addr")" || bad "m$i 启动失败"
  done
  sleep 10

  # mesh 形成：boot connected≥4
  local c0
  c0="$(curl -fsS 127.0.0.1:5002/peers | jq -r '.connected_peers')"
  echo "   mesh boot connected_peers=$c0"
  [ "${c0:-0}" -ge 4 ] && ok "5 节点 mesh 形成（boot 连 4 节点）" || bad "mesh 未充分形成（$c0）"

  # 广播延迟：从一个叶子节点(m4 api=5042)写，测量 boot 收到的时间
  local m4_api=5042 val="lat-$RANDOM"
  local t0 t1 ms
  curl -s -o /dev/null -X POST "127.0.0.1:$m4_api/api/v1/crdt" \
    -H "Authorization: Bearer $BEARER" -H 'Content-Type: application/json' \
    -d "{\"key\":\"lat\",\"value\":\"$val\"}"
  t0="$(date +%s%3N)"
  if wait_cond 5002 /api/v1/crdt/lat "\"value\":\"$val\"" 30 >/dev/null; then
    t1="$(date +%s%3N)"; ms=$((t1-t0))
    ok "GossipSub 广播到达 boot，延迟约 ${ms}ms（写入到观测）"
  else bad "广播 30s 内未到达 boot"; fi
}

echo "工作目录: $AU_REG_DIR"
echo "二进制:   $GSN_BIN"
$GSN_BIN --version 2>&1 | head -2 || true
echo

case "${ONLY:-all}" in
  3) run_three ;;
  5) run_five ;;
  *) run_three; run_five ;;
esac

echo
echo "==================== 汇总 ===================="
echo "PASS=$PASS  FAIL=$FAIL"
if [ "$FAIL" -eq 0 ]; then
  echo "🎉 多节点回归全部通过"
  exit 0
else
  echo "⚠️ 存在失败项，见上"
  exit 1
fi

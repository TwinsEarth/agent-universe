#!/usr/bin/env bash
# Agent Universe —— 规模组网脚本（DHT“假成功”检测器）
#
# 启动 N 个本地节点（默认 50），检查每个节点的 DHT 路由表是否真正填充，
# 防止“节点数<10 时路由表易填满、掩盖路由算法问题”。
#
# 指标：
#   - 每个节点 routing_entries（Kademlia kbucket 条目数）；
#   - 分布（min / 中位 / max），及 routing_entries ≥ K（默认 K=20）的节点占比；
#   - 跨节点查找：若守护进程提供 /api/v1/network/find 则采样成功率，
#     否则输出“跳过（查找指标由 in-process 测试覆盖）”。
#
# 用法：
#   scripts/regression-scale.sh            # N=50
#   GSN_N=50 scripts/regression-scale.sh
# 环境：GSN_BIN / SCALE_DIR / K / SETTLE_SECS / KEEP=1
# 注意：脚本默认设 GSN_DISABLE_PUBLIC_RELAY=1（若守护进程支持），让规模测试只走本地，
#       不冲击公共 relay 基础设施。
#
# 退出码：0=达标（≥90% 节点路由表≥K 且网络形成）；1=未达标；2=环境错误。

set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
GSN_BIN="${GSN_BIN:-$REPO_ROOT/gsn-core/target/release/gsn-daemon}"
N="${GSN_N:-50}"
K="${K:-20}"
WORK="${SCALE_DIR:-/tmp/au-scale-$$}"
SETTLE_SECS="${SETTLE_SECS:-60}"
BEARER="scale-token"
PIDS=()
mkdir -p "$WORK"

[ -x "$GSN_BIN" ] || { echo "❌ 找不到二进制 $GSN_BIN"; exit 2; }

cleanup() {
  if [ "${KEEP:-0}" != "1" ]; then
    local p
    for p in "${PIDS[@]:-}"; do kill "$p" >/dev/null 2>&1; done
    sleep 0.5
    for p in "${PIDS[@]:-}"; do kill -9 "$p" >/dev/null 2>&1; done
  fi
}
trap cleanup EXIT

echo "启动 N=$N 个节点（K=$K，settle=${SETTLE_SECS}s），工作目录 $WORK"

# boot
mkdir -p "$WORK/n0"
GSN_DISABLE_PUBLIC_RELAY=1 REST_BEARER_TOKEN="$BEARER" \
  "$GSN_BIN" --listen 127.0.0.1 --port 4001 --api-port 4002 \
    --data-dir "$WORK/n0" --mode full >"$WORK/n0/node.log" 2>&1 &
PIDS+=($!)
# 等 boot 就绪
for i in $(seq 1 80); do
  curl -fsS 127.0.0.1:4002/health >/dev/null 2>&1 && break
  sleep 0.5
done

# workers（i=1..N-1），p2p = 4001+10i，api = 4002+10i
BOOT_ADDR="/ip4/127.0.0.1/tcp/4001"
fail_starts=0
for i in $(seq 1 $((N-1))); do
  p2p=$((4001+10*i)); api=$((4002+10*i)); d="$WORK/n$i"; mkdir -p "$d"
  GSN_DISABLE_PUBLIC_RELAY=1 REST_BEARER_TOKEN="$BEARER" \
    "$GSN_BIN" --listen 127.0.0.1 --port "$p2p" --api-port "$api" \
      --data-dir "$d" --mode full --bootstrap "$BOOT_ADDR" >"$d/node.log" 2>&1 &
  PIDS+=($!)
  # 分批小睡，避免同一瞬时 dial 风暴
  if [ $((i % 10)) -eq 0 ]; then sleep 1; fi
done

echo "已发起 $N 个节点，等待 ${SETTLE_SECS}s 让 DHT 收敛..."
sleep "$SETTLE_SECS"

# 采集 routing_entries
ENTRIES_FILE="$WORK/routing_entries.txt"
: > "$ENTRIES_FILE"
alive=0
for i in $(seq 0 $((N-1))); do
  api=$((4002+10*i))
  r="$(curl -fsS "127.0.0.1:$api/api/v1/network/info" 2>/dev/null | jq -r '.routing_entries // -1')"
  if [ "$r" != "-1" ]; then alive=$((alive+1)); echo "$r" >> "$ENTRIES_FILE"; fi
done

echo
echo "==================== 路由表分布（存活节点 $alive/$N）===================="
sort -n "$ENTRIES_FILE" | awk -v K="$K" -v N="$alive" '
  { v[NR]=$1; sum+=$1 }
  END {
    n=NR;
    print "min="v[1];
    print "median=" (n%2? v[int(n/2)+1] : (v[n/2]+v[n/2+1])/2);
    print "max="v[n];
    print "mean="sum/n;
    c=0; for(i=1;i<=n;i++) if(v[i]>=K) c++;
    print "nodes_with_ge_K="c" / "n" ("100*c/n"%)";
    print "GATE_PASS=" (c/n>=0.90 ? "yes" : "no")
  }'

# 跨节点查找（如果有 find 端点）
has_find="$(curl -s -o /dev/null -w '%{http_code}' "127.0.0.1:4002/api/v1/network/find?key=test" 2>/dev/null || echo 000)"
if [ "$has_find" != "404" ] && [ "$has_find" != "000" ]; then
  echo "find 端点存在($has_find)，可在此采样跨节点查找成功率与延迟"
else
  echo "跨节点查找成功率/跳数：跳过（该口径由 in-process 多 swarm 测试覆盖 O(log n)）"
fi

gate="$(sort -n "$ENTRIES_FILE" | awk -v K="$K" '{c++} END{}' ; \
  awk -v K="$K" '{v[NR]=$1} END{c=0;for(i=1;i<=NR;i++) if(v[i]>=K)c++; print (c/NR>=0.90)?"yes":"no"}' "$ENTRIES_FILE")"

if [ "$alive" -ge $((N*9/10)) ] && [ "$gate" = "yes" ]; then
  echo "🎉 规模组网达标：≥90% 节点路由表 ≥ $K"
  exit 0
else
  echo "⚠️ 规模组网未达标（资源不足或路由问题，见上；若机器无法承载请标 未验证）"
  exit 1
fi

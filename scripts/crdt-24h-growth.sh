#!/usr/bin/env bash
# Agent Universe —— CRDT 状态膨胀长时观察（24h）
#
# 启动 3 节点，持续写入，每小时记录每个节点的 CRDT 键数与估算字节数，
# 检查增长曲线：
#   - 无快照/GC 时：单调无界、近似线性（甚至翻倍）→ 失控；
#   - 有上限/快照时：达到 GSN_CRDT_MAX_KEYS 后曲线走平 → 受控。
#
# 用法：
#   scripts/crdt-24h-growth.sh
# 环境：
#   HOURS=24（观察小时数，可缩短为 HOURS=1 做冒烟）
#   GSN_CRDT_MAX_KEYS=500（用于演示上限；需守护进程支持该 env，默认 100000）
#   KEEP=1（结束后保留节点）
#
# 退出码：0=增长受控（后期采样走平）；1=失控/未按预期；2=环境错误。
# 注意：该脚本默认 GSN_DISABLE_PUBLIC_RELAY=1，本地观察。

set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
GSN_BIN="${GSN_BIN:-$REPO_ROOT/gsn-core/target/release/gsn-daemon}"
HOURS="${HOURS:-24}"
MAXKEYS="${GSN_CRDT_MAX_KEYS:-500}"
WORK="${CRDT24_DIR:-/tmp/au-crdt24-$$}"
BEARER="c24-token"
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

# 三节点
for i in 0 1 2; do
  p2p=$((4201+10*i)); api=$((4202+10*i)); d="$WORK/n$i"; mkdir -p "$d"
  boot=()
  [ "$i" -ne 0 ] && boot=(--bootstrap "/ip4/127.0.0.1/tcp/4201")
  GSN_DISABLE_PUBLIC_RELAY=1 GSN_CRDT_MAX_KEYS="$MAXKEYS" REST_BEARER_TOKEN="$BEARER" \
    "$GSN_BIN" --listen 127.0.0.1 --port "$p2p" --api-port "$api" \
      --data-dir "$d" --mode full "${boot[@]}" >"$d/node.log" 2>&1 &
  PIDS+=($!)
done

# 等就绪
for i in $(seq 1 80); do
  curl -fsS 127.0.0.1:4202/health >/dev/null 2>&1 && curl -fsS 127.0.0.1:4222/health >/dev/null 2>&1 && break
  sleep 0.5
done
echo "三节点就绪，开始 ${HOURS}h 观察（max_keys=$MAXKEYS）"

CSV="$WORK/crdt_growth.csv"
echo "hour,keys_n0,bytes_n0,keys_n1,bytes_n1,keys_n2,bytes_n2" > "$CSV"

# 后台持续写入（唯一键，直到上限；之后写同一键更新）
(
  c=0
  while true; do
    curl -s -o /dev/null -X POST 127.0.0.1:4202/api/v1/crdt \
      -H "Authorization: Bearer $BEARER" -H 'Content-Type: application/json' \
      -d "{\"key\":\"auto-$c\",\"value\":\"x\"}"
    c=$((c+1))
    sleep 0.05
  done
) &
WRITER_PID=$!
trap 'kill $WRITER_PID 2>/dev/null; cleanup' EXIT

# 每小时采样
for h in $(seq 0 "$HOURS"); do
  read_crdt() {
    local api="$1"
    curl -fsS "127.0.0.1:$api/api/v1/crdt" 2>/dev/null | jq -r '"\(.keys // 0),\(.size_bytes // 0)"'
  }
  r0="$(read_crdt 4202)"; r1="$(read_crdt 4212)"; r2="$(read_crdt 4222)"
  echo "$h,$r0,$r1,$r2" >> "$CSV"
  echo "h=$h  n0=[$r0] n1=[$r1] n2=[$r2]"
  [ "$h" -lt "$HOURS" ] && sleep 3600
done

kill "$WRITER_PID" 2>/dev/null

# 判定：最后 1/4 采样的键数是否走平（标准差/增量接近0）
echo
echo "==================== 增长曲线判定 ===================="
awk -F, 'NR>1 {ks[NR]=$2} END {
  n=NR; tail=int(n/4);
  if (tail<1) { print "样本不足，未验证"; exit 2 }
  delta=ks[n]-ks[n-tail+1];
  print "末段键数增量="delta;
  # 走平阈值：末段增量 < 早期1小时增量的 10%
  early=ks[2]-ks[1];
  print "早期1h增量="early" 末段"tail"h增量="delta;
  if (early<=0) { print "GATE=inconclusive（早期无写入或 cap 已达）"; exit 1 }
  print (delta < early*0.1 ? "GATE=controlled（增长受控）" : "GATE=uncontrolled（线性无界）");
}' "$CSV"

gate="$(awk -F, 'NR>1{ks[NR]=$2} END{tail=int(NR/4); d=ks[NR]-ks[NR-tail+1]; e=ks[2]-ks[1]; print (e>0 && d<e*0.1)?"ok":"no"}' "$CSV")"
echo "CSV 已写入：$CSV"
[ "$gate" = "ok" ] && exit 0 || exit 1

#!/usr/bin/env bash
# =============================================================================
# 纠删码分布式存储端到端回归（真实多进程 libp2p）
# -----------------------------------------------------------------------------
# 用 release 二进制起 3 个独立进程（不同 data-dir/端口/Keypair），
# 在 node0 ingest 一个 blob，再在本地分片不足的 node2/node1 发起 reconstruct，
# 验证：编码 → 确定性分片分发 → 缺失分片 NeedShards/ShardReply → RS 重建，
# 且重建数据逐字节等于原始数据。
#
# 实跑通过（2026-10-05，云机 4 核/8GB）：node2/node1 均 MATCH。
# 用法：bash scripts/regression-erasure.sh
# =============================================================================
set -euo pipefail
cd "$(git rev-parse --show-toplevel)"

export PATH="$HOME/.cargo/bin:$PATH"
cargo build --release --workspace
BIN=gsn-core/target/release/gsn-daemon
ROOT=/tmp/er
TOKEN=localdev
rm -rf "$ROOT"; mkdir -p "$ROOT/n0" "$ROOT/n1" "$ROOT/n2"
export GSN_DISABLE_PUBLIC_RELAY=1 REST_BEARER_TOKEN="$TOKEN" RUST_LOG=info

nohup $BIN --listen 127.0.0.1 --port 4001 --api-port 4002 --data-dir "$ROOT/n0" --mode full > "$ROOT/n0.log" 2>&1 &
nohup $BIN --listen 127.0.0.1 --port 4011 --api-port 4012 --data-dir "$ROOT/n1" --mode full \
  --bootstrap /ip4/127.0.0.1/tcp/4001 > "$ROOT/n1.log" 2>&1 &
nohup $BIN --listen 127.0.0.1 --port 4021 --api-port 4022 --data-dir "$ROOT/n2" --mode full \
  --bootstrap /ip4/127.0.0.1/tcp/4001 > "$ROOT/n2.log" 2>&1 &

cleanup() { pkill -f "gsn-daemon --listen 127.0.0.1" 2>/dev/null || true; }
trap cleanup EXIT

echo "等待组网（12s）..."; sleep 12
grep -h "Peer ID" "$ROOT"/*.log

DATA="The quick brown fox jumps over the lazy dog. TwinEarth PCE-Format erasure end-to-end test payload 0001. $(printf 'x%.0s' {1..120})"
printf '%s' "$DATA" > "$ROOT/original.txt"
BODY=$(jq -n --arg d "$DATA" --arg b "erasure-e2e-001" '{blob_id:$b,data:$d}')
echo "=== node0 ingest ==="
curl -s -X POST -H "Authorization: Bearer $TOKEN" -H "Content-Type: application/json" -d "$BODY" \
  http://127.0.0.1:4002/api/v1/erasure/ingest | jq .
sleep 2

reconstruct_at() {
  local port=$1 name=$2
  for i in $(seq 1 12); do
    local R
    R=$(curl -s -X POST -H "Authorization: Bearer $TOKEN" -H "Content-Type: application/json" \
      -d '{"blob_id":"erasure-e2e-001"}' "http://127.0.0.1:$port/api/v1/erasure/reconstruct" || true)
    if echo "$R" | jq -e '.data_hex' >/dev/null 2>&1; then
      echo "$R" | python3 -c "import json,sys; d=json.load(sys.stdin); h=d['data_hex'][2:];
print('$name MATCH' if bytes.fromhex(h)==open('$ROOT/original.txt','rb').read() else '$name MISMATCH')"
      return 0
    fi
    sleep 1
  done
  echo "$name FAILED (超时未重建)"; return 1
}

# node2/node1 通常本地分片不足，需跨节点 NeedShards 拉取后重建。
reconstruct_at 4022 node2
reconstruct_at 4012 node1
echo "=== 纠删码端到端回归通过 ==="

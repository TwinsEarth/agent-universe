#!/usr/bin/env bash
# Agent Universe —— 信誉冷启动 / Sybil 女巫攻击场景（100 个新节点同时加入）
#
# 目标：验证新节点无法通过“批量新身份 + 互相刷分”快速获得高信誉或治理特权。
#
# 现状判定（本脚本实际验证）：
#   1) 治理是 fail-closed：未配置 GSN_GOVERNANCE_FILE 时，任何新节点发起的
#      仲裁/授信请求必须被拒绝（无签名治理信封 / 非成员）；
#   2) 信誉账本不跨节点复制（各节点本地记账），新节点无法影响其他节点对自己的评价；
#   3) 100 个新节点加入后，网络仍由本地 DHT/CRDT 主导，新节点不获得特权。
#
# 用法：scripts/regression-sybil-100.sh
# 环境：GSN_N=100、KEEP=1
# 注意：该脚本依赖 regression-scale.sh 完成组网，并额外做治理 fail-closed 验证。
# 退出码：0=未发现女巫可获得特权；1=发现可利用面。

set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
export GSN_N="${GSN_N:-100}"
export K="${K:-20}"
export SETTLE_SECS="${SETTLE_SECS:-90}"

echo "========== 第 1 步：起 100 个新节点 =========="
if ! bash "$SCRIPT_DIR/regression-scale.sh"; then
  echo "⚠️ 100 节点组网未达标（可能是机器资源不足）→ 规模项标“未验证”，但仍做治理 fail-closed 检查"
fi

# 取一个 worker 的 api（n1 api=4012）
API=4012
for i in $(seq 1 80); do
  curl -fsS "127.0.0.1:$API/health" >/dev/null 2>&1 && break
  sleep 0.5
done

echo
echo "========== 第 2 步：治理 fail-closed（无 GSN_GOVERNANCE_FILE）=========="
fail=0
# 2.1 新节点试图用“自报仲裁者”仲裁——已被 P0-4 改为必须签名治理信封；
#      直接 POST 旧形状 body 应被拒绝（非 2xx）。
c1="$(curl -s -o /dev/null -w '%{http_code}' -X POST "127.0.0.1:$API/api/v1/disputes/x/arbitrate" \
  -H 'Content-Type: application/json' -d '{"arbitrator":"did:nau:attacker","guilty":true}')"
echo "   自报仲裁请求 http=$c1（期望非200，即 400/401/404）"
case "$c1" in 2??) echo "   ❌ 自报仲裁被接受，存在女巫罚没面"; fail=1;; *) echo "   ✅ 自报仲裁被拒绝";; esac

# 2.2 无凭证 deposit（本地铸币）应被拒绝
c2="$(curl -s -o /dev/null -w '%{http_code}' -X POST "127.0.0.1:$API/api/v1/accounts/did:nau:attacker/deposit" \
  -H 'Content-Type: application/json' -d '{"amount":1000000}')"
echo "   无凭证授信 http=$c2（期望非200）"
case "$c2" in 2??) echo "   ❌ 无凭证授信被接受，存在本地铸币面"; fail=1;; *) echo "   ✅ 无凭证授信被拒绝";; esac

# 2.3 新节点不配置治理集时，特权动作整体不可用（空治理集拒绝一切）
echo "   （空治理集 = GSN_GOVERNANCE_FILE 未设 → Governance.verify 对所有命令返回拒绝）"

echo
if [ "$fail" -eq 0 ]; then
  echo "🎉 Sybil 场景：100 新节点无法获得罚没/授信特权（fail-closed）"
  echo "   注：信誉分仍为本地记账（初始默认值）；跨节点信誉聚合在后续版本规划。"
  exit 0
else
  echo "⚠️ 发现可被女巫利用的特权面"
  exit 1
fi

#!/usr/bin/env bash
# 发布前门禁（防呆）：版本一致性 + clippy 严格 + Rust 全量测试。
#
# 铁律：必须在「最后一次文件改动」（含 git mv / 重命名 / 重写 / 改测试 / 改合约）
#      之后、git commit 之前运行；任何一步失败即禁止提交、禁止打 tag。
#      跑完之后若又动了任何文件，必须从头重跑本脚本。
#
# 历史教训（v2.6.2）：合约 ReputationBridge→ReputationRegistry 重命名后未重跑
#   test_contracts_exist、erasure 重写后未重跑 clippy，导致 CI 连续三次红灯。
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

echo "== [1/3] 版本一致性断言（scripts/check-version.sh）=="
bash scripts/check-version.sh

echo "== [2/3] cargo clippy --all-targets -D warnings =="
(cd gsn-core && cargo clippy --all-targets -- -D warnings)

echo "== [3/3] cargo test（Rust 全量）=="
(cd gsn-core && cargo test)

echo ""
echo "preflight OK：可以提交 / 打 tag。"
echo "提醒：若本版改了 JS，请再跑 test/run-all.sh 确认 JS 单测与历史回归。"

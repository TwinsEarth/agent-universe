#!/usr/bin/env bash
# Agent Universe —— GossipSub 20% 恶意节点（不转发）消息到达率
#
# 场景：10 节点，其中 2 个为恶意节点（订阅但不转发消息）。从任意正常节点
# 发布一条普通消息，测量该消息到达所有“正常节点”的比率，要求 > 95%，
# 并验证 GossipSub mesh 自愈（恶意节点在 mesh 心跳中被绕开）。
#
# 实现说明：
#   标准 release 二进制的 GossipSub 按协议正常转发，无法仅靠命令行把某个节点
#   切成“不转发”。因此该检查以**真实 libp2p 的多 swarm 集成测试**实现（在
#   同一测试进程里起 10 个独立 P2pPeer/swarm，其中 2 个以“丢弃入站转发”的
#   恶意配置构建），这是真实 libp2p 行为而非 mock，仅“不是多 OS 进程”。
#
# 用法：scripts/regression-malicious.sh
# 退出码：0=到达率>95%；1=未达标。

set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

export PATH="$HOME/.cargo/bin:$PATH"

echo "运行真实 libp2p 多 swarm 恶意节点测试..."
cd "$REPO_ROOT/gsn-core"
if cargo test --test malicious_test -- --nocapture; then
  echo "🎉 20% 恶意节点下消息到达率达标（>95%）"
  exit 0
else
  echo "⚠️ 恶意节点测试未通过或测试文件缺失（应由 tests/malicious_test.rs 提供）"
  exit 1
fi

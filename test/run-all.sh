#!/usr/bin/env bash
# test/run-all.sh — Agent Universe 一键全量回归（Linux / macOS）
# 依次跑：JS SDK 单测、历史 bug 回归、Rust gsn-core 全量测试。
set -u

cd "$(dirname "$0")/.."
ROOT="$(pwd)"

# 确保能找到 cargo（用户级安装，未进全局 PATH 的环境）
if [ -f "$HOME/.cargo/env" ]; then
  . "$HOME/.cargo/env"
fi
export PATH="$HOME/.cargo/bin:$PATH"

fail=0

echo "=== 1/3 JS SDK 单元测试 ==="
node js/test/test.js || fail=$((fail+1))

echo ""
echo "=== 2/3 历史 Bug 回归（test/regression.js）==="
node test/regression.js || fail=$((fail+1))

echo ""
echo "=== 3/3 Rust gsn-core 全量测试 ==="
( cd gsn-core && cargo test ) || fail=$((fail+1))

echo ""
if [ "$fail" -eq 0 ]; then
  echo "OK 全部测试通过"
  exit 0
else
  echo "FAIL 有 $fail 组测试失败"
  exit 1
fi

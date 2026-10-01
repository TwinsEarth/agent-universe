#!/usr/bin/env bash
# check-version.sh — 版本一致性断言（GAP §9.3）
# 仓库根 VERSION 是唯一权威来源；本脚本断言所有包的版本声明都与之一致，
# 任何漂移以非 0 退出（CI 必跑，漂移即红）。
# 用法: bash scripts/check-version.sh
set -uo pipefail

cd "$(dirname "$0")/.."

VERSION="$(tr -d '[:space:]' < VERSION)"
if [[ ! "$VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
  echo "FAIL: VERSION 文件内容非法: '$VERSION'（应为 X.Y.Z）"; exit 2
fi
IFS='.' read -r X Y Z <<< "$VERSION"
RUST="0.$X.$((Y*10+Z))"

fail=0

# first_version <file> [grep-pattern] : 取匹配行里第一个三段版本号
first_version() {
  local f="$1" pat="${2:-}"
  if [ -n "$pat" ]; then grep -m1 "$pat" "$f" 2>/dev/null
  else cat "$f" 2>/dev/null; fi \
    | grep -oE '[0-9]+\.[0-9]+\.[0-9]+' | head -1
}

expect() {
  local label="$1" want="$2" got="$3"
  if [ "$got" != "$want" ]; then
    echo "FAIL: $label 期望 $want，实际 ${got:-<缺失>}"; fail=1
  else
    echo "ok:   $label = $got"
  fi
}

echo ">>> 权威 VERSION=$VERSION  Rust gsn-core=$RUST"

# npm / JS / Python / Tauri 壳（三段，== VERSION）
expect "根 package.json"            "$VERSION" "$(first_version package.json '"version"')"
expect "js/package.json"            "$VERSION" "$(first_version js/package.json '"version"')"
expect "client/package.json"        "$VERSION" "$(first_version client/package.json '"version"')"
expect "desktop/package.json"       "$VERSION" "$(first_version desktop/package.json '"version"')"
expect "aip-sdk-py/pyproject.toml"  "$VERSION" "$(first_version aip-sdk-py/pyproject.toml '^version')"
expect "aip/aip/__init__.py"        "$VERSION" "$(first_version aip-sdk-py/aip/__init__.py '__version__')"
expect "client/src-tauri Cargo.toml"  "$VERSION" "$(first_version client/src-tauri/Cargo.toml '^version')"
expect "desktop/src-tauri Cargo.toml" "$VERSION" "$(first_version desktop/src-tauri/Cargo.toml '^version')"

# gsn-core Rust 线（0.X.NN）
expect "gsn-core/Cargo.toml"        "$RUST" "$(first_version gsn-core/Cargo.toml '^version')"

# ci.yml 里 js-test 的 root re-export 版本断言（硬编码版本号，bump 必须改到，否则 js-test 红）
expect "ci.yml 版本断言" "$VERSION" "$(first_version .github/workflows/ci.yml 'au.version')"

if [ "$fail" -ne 0 ]; then
  echo ">>> 版本一致性检查失败：请先 bash scripts/bump-version.sh $VERSION 或修正 VERSION"
  exit 1
fi
echo ">>> 版本一致性检查通过"

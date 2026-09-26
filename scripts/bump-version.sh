#!/usr/bin/env bash
# bump-version.sh — 一键把全仓版本声明点改到新版本
# 用法: bash scripts/bump-version.sh 2.5.6
# 规则: npm X.Y.Z  <->  Rust gsn-core 0.2.(Y*10+Z)
# 配套清单: docs/version-checklist.md （新增版本点时务必同步更新本脚本与清单）
set -euo pipefail

NEW="$1"
if [[ ! "$NEW" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
  echo "用法: $0 X.Y.Z  (例如 $0 2.5.6)"; exit 1
fi
IFS='.' read -r X Y Z <<< "$NEW"
RUST="0.2.$((Y*10+Z))"
echo ">>> npm 版本: $NEW   Rust gsn-core 版本: $RUST"

cd "$(dirname "$0")/.."

V="[0-9]+\\.[0-9]+\\.[0-9]+"   # 匹配任意 X.Y.Z

# ── A. npm / JS 包 ──
for f in package.json js/package.json js/package-lock.json client/package.json client/package-lock.json desktop/package.json desktop/package-lock.json; do
  sed -i "s/\"version\": \"$V\"/\"version\": \"$NEW\"/g" "$f"
done
sed -i "s/const version = '$V'/const version = '$NEW'/" js/index.js
sed -i "s/version: opts.version || '$V'/version: opts.version || '$NEW'/" js/lib/aca.js
sed -i "s/agent-universe-js', version: '$V'/agent-universe-js', version: '$NEW'/" js/lib/mcp.js
sed -i "s/test('版本号为 $V'/test('版本号为 $NEW'/" js/test/test.js
sed -i "s/assert.strictEqual(version, '$V'/assert.strictEqual(version, '$NEW'/" js/test/test.js

# ── B/C. Tauri client/desktop ──
for c in client desktop; do
  sed -i "s/^version = \"$V\"/version = \"$NEW\"/" $c/src-tauri/Cargo.toml
  sed -i "s/v$V 跨平台客户端/v$NEW 跨平台客户端/; s/v$V 桌面客户端/v$NEW 桌面客户端/" $c/src-tauri/Cargo.toml
  sed -i "s/\"version\": \"$V\"/\"version\": \"$NEW\"/g" $c/src-tauri/tauri.conf.json
  sed -i "s/Agent Universe v$V/Agent Universe v$NEW/g" $c/src-tauri/tauri.conf.json
  sed -i "s/\"$V\"\.to_string()/\"$NEW\".to_string()/" $c/src-tauri/src/lib.rs
done
sed -i "s/v$V — 桌面客户端前端/v$NEW — 桌面客户端前端/" desktop/src/main.js

# ── D. gsn-core (Rust 线) ──
sed -i "s/^version = \"0\.2\.$V\"/version = \"$RUST\"/" gsn-core/Cargo.toml
sed -i "s|核心库 v$V:|核心库 v$NEW:|" gsn-core/Cargo.toml
sed -i "s/println!(\"agent-universe v$V\")/println!(\"agent-universe v$NEW\")/" gsn-core/src/bin/gsn.rs
sed -i "s|^//! Agent Universe gsn-core v$V|//! Agent Universe gsn-core v$NEW|" gsn-core/src/lib.rs

# ── D2. Cargo.lock 本包版本（按包名块改，不误伤其他依赖；lock 不存在则跳过）──
bump_lock() {
  local lock="$1" pkg="$2" newver="$3"
  [ -f "$lock" ] || return 0
  awk -v pkg="$pkg" -v newver="$newver" '
    $0=="name = \""pkg"\"" {print; getline; sub(/version = .*/, "version = \""newver"\""); print; next}
    {print}
  ' "$lock" > "$lock.tmp" && mv "$lock.tmp" "$lock"
}
bump_lock gsn-core/Cargo.lock gsn-core "$RUST"
bump_lock client/src-tauri/Cargo.lock au-client-universal "$NEW"
bump_lock desktop/src-tauri/Cargo.lock au-client "$NEW"

# ── E. Python SDK ──
sed -i "s/^version = \"$V\"/version = \"$NEW\"/" aip-sdk-py/pyproject.toml

# ── F. CI ──
sed -i "s/au.version !==\?'$V'/au.version!=='$NEW'/" .github/workflows/ci.yml

# ── 客户端下载链接 / 文件名 ──
sed -i "s/v$V 跨平台客户端/v$NEW 跨平台客户端/" client/README.md
sed -i "s/_$V_/_${NEW}_/g" client/README.md
sed -i "s/-v$V\.tar\.gz/-v${NEW}.tar.gz/g" client/README.md
sed -i "s|releases/tag/v$V|releases/tag/v$NEW|g" client/README.md client/platforms/*.md

echo ">>> 完成。下一步:"
echo "    node js/test/test.js"
echo "    补 README/RELEASES/CHANGELOG/releases/v$NEW.md (见 docs/version-checklist.md 第二节)"

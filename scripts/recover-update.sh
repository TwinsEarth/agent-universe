#!/bin/sh
# 带外恢复/手动升级 agent-universe (gsn-daemon)。
#
# 用途：当本机 gsn 版本无法自动更新（典型为 v3.6.1——其更新器无法解析域名，
#       且默认 minor 通道不追 patch），用本脚本一次性恢复到可自动更新的版本。
#       v3.6.2 起更新器已修复 DNS；v3.6.3 起支持 HTTPS_PROXY 与 musl/老 glibc 资产。
#
# 两种方式（默认二进制；可用 --npm 走 npm 全局安装）：
#   sh scripts/recover-update.sh                 # 二进制方式，恢复到脚本内置目标版本
#   sh scripts/recover-update.sh --version 3.6.3
#   sh scripts/recover-update.sh --npm [--version X.Y.Z]
#
# 环境：尊重 HTTPS_PROXY/https_proxy（curl 自带支持）；需要 curl 与 sha256sum
#       （macOS 无 sha256sum 时回退 shasum -a 256）。
#
# 安全：替换前强制校验 GitHub 随附 .sha256；校验不过绝不覆盖；先备份再原子 rename。
#       Windows 请在停止运行中的节点后，直接下载 .exe 资产或使用 npm（本脚本面向 unix）。

set -eu

REPO="TwinsEarth/agent-universe"
BASE="https://github.com/${REPO}/releases/download"
PKG="@twinsearth/agent-universe"
# 内置恢复目标：第一个“更新器可联网自愈”的稳定线版本。
DEFAULT_TARGET="3.6.3"

TARGET=""
USE_NPM=0
DEST=""

while [ $# -gt 0 ]; do
  case "$1" in
    --version) TARGET="${2:?--version 需要一个版本号}"; shift 2 ;;
    --npm) USE_NPM=1; shift ;;
    --dest) DEST="${2:?--dest 需要一个路径}"; shift 2 ;;
    -h|--help)
      sed -n '2,20p' "$0"; exit 0 ;;
    *) echo "未知参数: $1" >&2; exit 2 ;;
  esac
done
[ -z "$TARGET" ] && TARGET="$DEFAULT_TARGET"

need() { command -v "$1" >/dev/null 2>&1 || { echo "缺少依赖: $1" >&2; exit 2; }; }
need curl

# ---- npm 路径：交给 npm 全局安装，天然跨平台且自带校验 ----------------------
if [ "$USE_NPM" -eq 1 ]; then
  need npm
  echo ">> 通过 npm 安装 ${PKG}@${TARGET} ..."
  exec npm install -g --no-fund --no-audit "${PKG}@${TARGET}"
fi

# ---- 二进制路径：下载 + sha256 校验 + 备份 + 原子替换 ----------------------
need chmod
if command -v sha256sum >/dev/null 2>&1; then
  sha() { sha256sum "$1" | awk '{print $1}'; }
elif command -v shasum >/dev/null 2>&1; then
  sha() { shasum -a 256 "$1" | awk '{print $1}'; }
else
  echo "缺少 sha256sum / shasum，无法校验" >&2; exit 2
fi

os="$(uname -s)"
arch="$(uname -m)"
case "$os/$arch" in
  Linux/x86_64)  ASSET="gsn-daemon-x86_64-unknown-linux-gnu" ;;
  Darwin/arm64)  ASSET="gsn-daemon-aarch64-apple-darwin" ;;
  *)
    echo "本脚本仅提供 Linux x86_64 / macOS arm64 的二进制恢复；" >&2
    echo "当前为 $os/$arch，请改用: sh $0 --npm --version $TARGET" >&2
    exit 2 ;;
esac

# 默认覆盖 PATH 中的 gsn-daemon；找不到则放到当前目录。
if [ -z "$DEST" ]; then
  if command -v gsn-daemon >/dev/null 2>&1; then
    DEST="$(command -v gsn-daemon)"
  elif command -v gsn >/dev/null 2>&1; then
    DEST="$(command -v gsn)"
  else
    DEST="./$ASSET"
    echo ">> 未在 PATH 发现已安装的 gsn，将下载到 $DEST"
  fi
fi

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
url_bin="${BASE}/v${TARGET}/${ASSET}"
url_sha="${url_bin}.sha256"

echo ">> 下载 ${ASSET} (v${TARGET})"
curl -fL --retry 2 --connect-timeout 15 -o "$work/bin" "$url_bin"
curl -fL --retry 2 --connect-timeout 15 -o "$work/sha" "$url_sha"

want="$(awk '{print $1}' "$work/sha")"
got="$(sha "$work/bin")"
if [ "$want" != "$got" ]; then
  echo "sha256 校验失败，放弃替换。" >&2
  echo "  want=$want" >&2
  echo "  got =$got" >&2
  exit 1
fi
echo ">> sha256 校验通过: $got"

chmod 0755 "$work/bin"
if [ -e "$DEST" ]; then
  cp -f "$DEST" "${DEST}.bak.$(date +%s)" 2>/dev/null || true
fi
# 同文件系统内原子替换。
mv -f "$work/bin" "$DEST"
echo ">> 已恢复 $DEST 到 v${TARGET}"
"$DEST" --version 2>/dev/null || "$DEST" version 2>/dev/null || true
echo ">> 完成。此后 gsn 的自动更新（含 DNS/代理）即可正常工作。"

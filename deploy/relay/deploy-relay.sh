#!/usr/bin/env bash
# =============================================================================
# Agent Universe —— 自建 Relay 中继服务端一键部署脚本
# -----------------------------------------------------------------------------
# 目标：公网云服务器（如 202.182.123.154）上部署 Circuit Relay v2 中继（hop），
#       打通 Mac↔Windows 跨网络跨版本互连（闸门③）。默认跨网连接方式为 P2P。
#
# 用法：
#   chmod +x deploy/relay/deploy-relay.sh
#   sudo ./deploy/relay/deploy-relay.sh [--port 4001] [--api-port 4002] [--bearer <token>]
#
# 前置：
#   - 本机已构建 release 二进制（cargo build --release --bin gsn-daemon）
#   - 已创建运行用户（或本脚本自动创建 au-relay）
# =============================================================================
set -euo pipefail

PORT=4001
API_PORT=4002
BEARER=""
DATA_DIR="/var/lib/au-relay/data"
BIN_DST="/usr/local/bin/gsn-daemon"

# 仓库根（脚本所在目录的上级上级）
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/../.." && pwd)"

usage() {
  echo "用法: $0 [--port 4001] [--api-port 4002] [--bearer <token>]"
  echo "  --port       libp2p 中继传输端口（客户端 --relay 指向）"
  echo "  --api-port   HTTP API 端口（健康检查）"
  echo "  --bearer     REST 写门 Bearer（生产必须设置高熵随机值）"
  exit 1
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --port) PORT="$2"; shift 2 ;;
    --api-port) API_PORT="$2"; shift 2 ;;
    --bearer) BEARER="$2"; shift 2 ;;
    *) usage ;;
  esac
done

[[ -z "$BEARER" ]] && BEARER="$(openssl rand -hex 24)" && echo "已自动生成 REST_BEARER_TOKEN: ${BEARER}（请妥善保存）"

echo "==> [1/5] 检查 release 二进制"
BIN_SRC="${REPO_ROOT}/gsn-core/target/release/gsn-daemon"
if [[ ! -x "$BIN_SRC" ]]; then
  echo "未找到 ${BIN_SRC}，请先构建："
  echo "  cd ${REPO_ROOT}/gsn-core && cargo build --release --bin gsn-daemon"
  exit 1
fi

echo "==> [2/5] 创建运行用户与数据目录"
id -u au-relay &>/dev/null || useradd --system --create-home --home-dir /var/lib/au-relay au-relay
mkdir -p "${DATA_DIR}"
chown -R au-relay:au-relay "${DATA_DIR}"

echo "==> [3/5] 安装二进制"
cp "${BIN_SRC}" "${BIN_DST}"
chmod 755 "${BIN_DST}"

echo "==> [4/5] 安装 systemd 单元"
sed -e "s|--port 4001|--port ${PORT}|" \
    -e "s|--api-port 4002|--api-port ${API_PORT}|" \
    -e "s|CHANGE_ME_relay_server_token_only|${BEARER}|" \
    "${SCRIPT_DIR}/systemd/agent-universe-relay.service" > /etc/systemd/system/agent-universe-relay.service
systemctl daemon-reload
systemctl enable agent-universe-relay

echo "==> [5/5] 启动中继服务端"
systemctl restart agent-universe-relay

echo ""
echo "================================================================"
echo "自建 Relay 中继服务端已部署"
echo "  libp2p  : /ip4/$(hostname -I | awk '{print $1}')/tcp/${PORT}"
echo "  HTTP API: http://<本机公网IP>:${API_PORT}/healthz"
echo ""
echo "获取中继 PeerId（客户端 --relay 需要）："
echo "  journalctl -u agent-universe-relay | grep 'p2p/' | tail -1"
echo ""
echo "客户端（Mac/Windows 任意机器）加入方式："
echo "  gsn-daemon --data-dir <dir> --relay /ip4/<SERVER_IP>/tcp/${PORT}/p2p/<RELAY_PEER_ID>"
echo "================================================================"

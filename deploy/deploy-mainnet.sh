#!/bin/bash
# GSN 主网一键部署脚本 (Mac mini)
set -e

echo "=========================================="
echo "  GSN 主网部署 - Mac mini"
echo "=========================================="

# 1. 系统检查
echo ""
echo "[1/8] 系统检查..."
if [[ "$(uname)" != "Darwin" ]]; then
    echo "❌ 此脚本仅支持 macOS"
    exit 1
fi
echo "✅ macOS 环境确认"

# 2. 电源策略
echo ""
echo "[2/8] 配置电源策略..."
bash "$(dirname "$0")/macos/pmset-setup.sh"

# 3. 创建数据目录
echo ""
echo "[3/8] 创建数据目录..."
sudo mkdir -p /Volumes/GSN/{data,logs}
sudo chown -R $(whoami):staff /Volumes/GSN
echo "✅ /Volumes/GSN 已创建"

# 4. 安装 gsn-daemon
echo ""
echo "[4/8] 安装 gsn-daemon..."
sudo cp target/release/gsn-daemon /usr/local/bin/
sudo chmod +x /usr/local/bin/gsn-daemon
echo "✅ gsn-daemon 已安装到 /usr/local/bin"

# 5. 配置 launchd
echo ""
echo "[5/8] 配置 launchd 服务..."
cp "$(dirname "$0")/macos/com.gsn.node.plist" ~/Library/LaunchAgents/
launchctl load ~/Library/LaunchAgents/com.gsn.node.plist
echo "✅ launchd 服务已加载"

# 6. 防火墙配置
echo ""
echo "[6/8] 配置防火墙..."
sudo /usr/libexec/ApplicationFirewall/socketfilterfw --setglobalstate on
echo "✅ 防火墙已开启"

# 7. 网络配置
echo ""
echo "[7/8] 配置网络..."
sudo scutil --set HostName gsn-mainnet
echo "✅ 主机名设置为 gsn-mainnet"

# 8. 验证
echo ""
echo "[8/8] 验证部署..."
sleep 3
if pgrep -x gsn-daemon > /dev/null; then
    echo "✅ gsn-daemon 运行中"
else
    echo "⚠️ gsn-daemon 未运行，检查日志: /Volumes/GSN/logs/"
fi

echo ""
echo "=========================================="
echo "  ✅ GSN 主网部署完成"
echo "=========================================="
echo "  P2P 端口: 4001"
echo "  API 端口: 4002"
echo "  数据目录: /Volumes/GSN/data"
echo "  日志目录: /Volumes/GSN/logs"
echo "=========================================="

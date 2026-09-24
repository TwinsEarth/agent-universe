#!/bin/bash
# GSN 主网 Mac mini 电源策略配置
set -e

echo "=== GSN 主网电源策略配置 ==="

# 关闭所有睡眠配置
sudo pmset -a sleep 0
sudo pmset -a disablesleep 1
sudo pmset -a hibernatemode 0
sudo pmset -a powernap 0
sudo pmset -a womp 1          # 网络唤醒
sudo pmset -a autorestart 1   # 断电恢复后自动开机

echo "=== 验证电源配置 ==="
pmset -g
pmset -g assertions

echo "✅ 电源策略配置完成"

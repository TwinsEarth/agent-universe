#!/usr/bin/env bash
# 判断给定版本是否为「客户端构建大版本」。
#
# 节奏（用户既定）：v2.7.0 生成一次客户端；之后每 9 个小版本后的一个大版本
# （v2.8.0、v2.9.0、v3.0.0 …）才重新构建/更新客户端，中间小版本复用已有客户端。
# 判定规则：版本号 >= v2.7.0 且 patch == 0。
#
# 用法: scripts/is-client-release.sh v2.7.0
# 退出码: 0 = 是客户端构建版本；1 = 否
set -euo pipefail

v="${1#v}"
major="${v%%.*}"
rest="${v#*.}"
minor="${rest%%.*}"
patch="${rest#*.}"

# patch 非 0 → 小版本，不构建客户端
if [ "$patch" -ne 0 ]; then
    exit 1
fi

# 三元组版本比较：(major,minor,patch) >= (2,7,0)
if [ "$major" -gt 2 ]; then
    exit 0
fi
if [ "$major" -lt 2 ]; then
    exit 1
fi
# major == 2
if [ "$minor" -ge 7 ]; then
    exit 0
fi
exit 1

# Agent Universe —— 自建 Relay 中继服务端部署指南

打通 **闸门③ Mac↔Windows 跨网络跨版本互连** 的关键基础设施：在公网云服务器上
运行 Circuit Relay v2 中继（hop），让任意网络下的节点经中继互通。默认跨网连接
方式为 **P2P**（直连优先，中继兜底）。

- 公网云服务器 IP：`202.182.123.154`（Vultr，Debian 11，x86_64）
- 中继传输端口：`4001/tcp + 4001/udp`（libp2p）
- HTTP API 端口：`4002/tcp`
- 中继 PeerId：`12D3KooWC2wm4Ru25bJDqwCfaq2Kedxc695XRjF5rhhU4aqceheq`

> **实机验证（2026-10-10，v3.9.13 / gsn-core 0.3.103）**：
> 中继服务端以官方 musl 静态二进制部署（服务器 glibc 2.31 不支持 GNU 产物），
> systemd 常驻（内存 ~18.5M），已自动接入 IPFS 公共网络（239+ 对等节点、
> DHT 172 路由条目）；客户端 `--relay /ip4/202.182.123.154/tcp/4001/p2p/<PeerId>`
> 实测 1.5s 直连中继、`ReservationReqAccepted` 建立 reservation（120s/131072B）、
> 获得 `/ip4/202.182.123.154/tcp/4001/p2p/<PeerId>/p2p-circuit/...` 可路由地址——
> **跨公网中继回路完整打通**。健康检查真实路径为 `/health` 与 `/api/v1/health`
> （`/healthz` 返回 404）。

---

## 一、快速开始（Docker，推荐）

```bash
# 仓库根构建镜像
docker build -t agent-universe/gsn-daemon:relay .

# 启动中继服务端
docker compose -f deploy/relay/docker-compose.yml up -d

# 查看日志（获取 PeerId 与启动确认）
docker compose -f deploy/relay/docker-compose.yml logs -f
# 期望看到：🔄 Relay Server 已启用: max_circuits=512 max_reservations=1024
```

健康检查：

```bash
curl -fsS http://202.182.123.154:4002/health
# {"hard":{"storage":"ok"},"mode":"relay","p2p_port":4001,"service":"gsn-daemon",
#  "soft":{"connected_peers":239,"crdt_keys":0,"crdt_size_bytes":0,"dht_routing_entries":172},
#  "status":"ok","uptime_ms":...,"version":"0.3.103"}
# 注意：/healthz 返回 404，真实路径是 /health 或 /api/v1/health
```

## 二、快速开始（systemd，生产）

```bash
chmod +x deploy/relay/deploy-relay.sh
sudo ./deploy/relay/deploy-relay.sh --bearer "$(openssl rand -hex 24)"

# 查看中继 PeerId
journalctl -u agent-universe-relay | grep 'p2p/' | tail -1
```

脚本自动完成：创建 `au-relay` 用户 → 安装二进制 → 安装并启用 systemd 单元 →
启动服务。手动安装方式见 [`systemd/agent-universe-relay.service`](systemd/agent-universe-relay.service)。

## 三、获取中继 PeerId（关键步骤）

中继 PeerId 是客户端 `--relay` 参数的必要组成。两种方式：

1. **日志**（最直接）：启动后第一行「🔄 Relay Server 已启用」下方的
   「其他节点请以 `--relay /ip4/<本机公网IP>/tcp/4001/p2p/<PEER_ID>` 加入」，
   或 `journalctl -u agent-universe-relay | grep 'p2p/'`。
2. **HTTP API**：`curl http://202.182.123.154:4002/health`（真实路径，`/healthz` 404）或 peer 信息端点。

PeerId 形如 `12D3KooW…`（libp2p Ed25519 身份，随 data-dir 持久化，重启不变）。

## 四、客户端加入（Mac / Windows / Linux 任意节点）

```bash
# 方式一：启动参数注入自建中继（推荐，写入配置后每次启动自动生效）
gsn-daemon \
  --data-dir /path/to/data \
  --relay /ip4/202.182.123.154/tcp/4001/p2p/<RELAY_PEER_ID>

# 方式二：多值，可同时注入公共中继 + 自建中继
gsn-daemon \
  --data-dir /path/to/data \
  --relay /ip4/202.182.123.154/tcp/4001/p2p/<RELAY_PEER_ID> \
  --relay /ip4/<public-relay-ip>/tcp/4001/p2p/<PUBLIC_RELAY_PEER_ID>
```

启动后日志验证：

```
→ 注入自建 relay: <RELAY_PEER_ID>
🛰️ 发起 relay 通道 ...
✅ relay reservation 已建立 <RELAY_PEER_ID>
```

reservation 建立后，本节点的 `p2p-circuit` 地址会经 identify 广播，其他节点即可
经自建中继找到你。

## 五、中继服务端运维

| 操作 | 命令 |
|---|---|
| 查看状态 | `systemctl status agent-universe-relay` |
| 实时日志 | `journalctl -u agent-universe-relay -f` |
| 重启 | `systemctl restart agent-universe-relay` |
| 健康检查 | `curl -fsS http://127.0.0.1:4002/health` |
| 中继事件 | `journalctl -u agent-universe-relay \| grep 'relay server:'` |

中继事件含义：

| 日志 | 含义 |
|---|---|
| `🔌 relay server: reservation 接受` | 有客户端在本中继上建立了 reservation（可被经此中继寻址） |
| `🔌 relay server: circuit 接受` | 有节点经此中继建立了 circuit 连接（中继正在转发流量） |

## 六、多节点同机部署注意事项（踩坑实录）

### 6.1 防火墙根因（v3.9.13 实机）
**现象**：客户端 `--relay` 注入自建中继后，日志出现
`出站连接失败 peer=<中继PeerId>: Transport(... Timeout)`，无法建立 reservation。
**根因**：服务器 UFW 处于 active 且 INPUT policy DROP，**4001 端口未放行**。
出站不受影响（中继能连出去 239 个公共节点），但外部直连 4001 被防火墙拦截。
**修复**：
```bash
ufw allow 4001/tcp && ufw allow 4001/udp && ufw allow 4002/tcp
```
放行后客户端 1.5s 直连中继，`ReservationReqAccepted` 建立 reservation，
获得 `p2p-circuit` 可路由地址。`deploy-relay.sh` 已内置此步骤。

### 6.2 多节点同机 API 端口冲突
多节点（含中继 + 客户端）部署在同一台机器上测试时，**HTTP API 默认端口 4002
冲突**（`Error: Address already in use (os error 98)`）。必须为每个进程指定不同
`--api-port`：

```bash
# 中继 r：API 4002
gsn-daemon --data-dir ./r --port 41001 --api-port 4002 --relay-server \
  --relay-max-circuits 64 --relay-max-reservations 128

# 客户端 A：API 4012
gsn-daemon --data-dir ./a --port 41002 --api-port 4012 \
  --relay /ip4/127.0.0.1/tcp/41001/p2p/<RELAY_PEER_ID>

# 客户端 B：API 4013
gsn-daemon --data-dir ./b --port 41003 --api-port 4013 \
  --relay /ip4/127.0.0.1/tcp/41001/p2p/<RELAY_PEER_ID>
```

## 七、安全注意事项

- `REST_BEARER_TOKEN` 必须设置为高熵随机值（`openssl rand -hex 24`），
  禁止使用示例占位值暴露公网。
- 中继服务端以独立非 root 用户（`au-relay`）运行，systemd 单元含
  `NoNewPrivileges` / `ProtectSystem` / `ProtectHome` 加固。
- 若服务器有防火墙（ufw/firewalld/云安全组），放行 `4001/tcp`、`4001/udp`、
  `4002/tcp`。
- 中继只做流量转发（hop），不落盘业务数据；涉及中继传输的安全审计仍需外部
  专业审计（REQUIRE EXTERNAL AUDIT）。

## 八、版本

- 引入版本：**v3.9.13**（gsn-core 0.3.103）
- 参数：`--relay-server` / `--relay <addr>` / `--relay-max-circuits` / `--relay-max-reservations`

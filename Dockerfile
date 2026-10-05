# syntax=docker/dockerfile:1
# =============================================================================
# Agent Universe / gsn-core 多阶段镜像
# -----------------------------------------------------------------------------
# 构建上下文 = 仓库根（docker build 必须在仓库根执行）：
#   docker build -t agent-universe/gsn-daemon .
#
# 阶段：
#   builder  —— rust:1.88-alpine，musl 静态编译（rusqlite bundled 需要 C 编译器 cc）
#   final    —— gcr.io/distroless/static-debian12:nonroot，仅含静态二进制 + CA 证书
#
# 产物：gsn-daemon（全节点守护进程）+ gsn（运维 CLI）。最终镜像无 shell、无包管理器。
# =============================================================================

# -----------------------------------------------------------------------------
# Stage 1: builder
# -----------------------------------------------------------------------------
FROM rust:1.88-alpine AS builder

# build-base 提供 gcc/ld/make；musl-dev 提供头文件；
# rusqlite "bundled" 会从源码编译 SQLite，必须有 cc；musl 工具链产出静态二进制。
RUN apk add --no-cache build-base musl-dev

WORKDIR /build

# 仅拷贝 gsn-core（构建上下文为仓库根）。先拷 manifest 以利用 Docker 层缓存：
# 但本仓库 gsn-core 是单 crate（非 workspace 多包），直接整目录拷贝即可。
COPY gsn-core ./gsn-core

# 固定 target 目录，便于在 final 阶段精确定位产物。
ENV CARGO_TARGET_DIR=/build/target

# 编译两个二进制：gsn-daemon（守护进程）与 gsn（运维 CLI）。
# 如只需守护进程，可改为： cargo build --release --bin gsn-daemon
RUN cargo build --release \
      --manifest-path gsn-core/Cargo.toml \
      --bin gsn-daemon \
      --bin gsn

# -----------------------------------------------------------------------------
# Stage 2: final —— distroless 静态镜像（nonroot, uid=65532）
# -----------------------------------------------------------------------------
# 选用 static-debian12:nonroot：
#   - 无 glibc、无 shell；运行静态 musl 二进制正合适；
#   - 自带 nonroot 用户（uid 65532）与 CA 根证书（rustls/webpki-roots 已内置根，
#     系统证书作为兜底）。
#
# 备注（兼容性备选，默认不启用）：
#   libp2p 的 quic/tcp 与 ring 在 musl 上通常可静态运行；若在特定架构上遇到
#   ring/quic 的静态链接问题，可改用「debian-slim builder → debian-slim runtime」
#   方案：把 builder 换成 rust:1.88-bookworm（动态链接 glibc），final 换成
#   gcr.io/distroless/cc-debian12:nonroot（带 libc），其余 COPY 路径不变。
#   本主方案坚持 musl 静态 → distroless/static，追求最小攻击面。
FROM gcr.io/distroless/static-debian12:nonroot AS final

# 数据/身份目录。容器内进程以此为 HOME（身份、db、日志、治理集均落盘于此）。
WORKDIR /data
ENV HOME=/data

# 数据目录属主设为 nonroot：docker 首次创建命名卷时会继承镜像里 /data 的属主，
# 否则卷默认为 root 所有，uid 65532 无法写盘（gsn-daemon 会 create_dir_all 并
# 在 /data 下写 identity.key / gsn.db / logs/）。
RUN mkdir -p /data && chown 65532:65532 /data

# 拷贝静态二进制。release profile 已 strip 符号（见 gsn-core/Cargo.toml）。
COPY --from=builder /build/target/release/gsn-daemon /usr/local/bin/gsn-daemon
COPY --from=builder /build/target/release/gsn        /usr/local/bin/gsn

# 端口模型（每节点）：
#   4001/tcp —— libp2p TCP 监听
#   4001/udp —— libp2p QUIC（UDP 打洞直连）
#   4002/tcp —— HTTP REST API
EXPOSE 4001/tcp 4001/udp 4002/tcp

# 数据/身份持久化挂载点。
VOLUME ["/data"]

# 健康检查：exec 形式（distroless/static 无 shell，不能用 SHELL 形式）。
# 契约：gsn-daemon --healthcheck 内部 GET $GSN_HEALTHCHECK_URL
# （默认 http://127.0.0.1:4002/health，2xx→退出码0，3s 超时）。
# 注意：该子命令由并行的健康检查改造落地；在合入前，此处 HEALTHCHECK 会因
# “未知选项”失败——届时以镜像实际可用为准（本机无 docker，未实际构建验证）。
HEALTHCHECK --interval=30s --timeout=5s --start-period=20s --retries=3 \
  CMD ["gsn-daemon", "--healthcheck"]

# 以 nonroot 运行（镜像虽自带 nonroot，这里显式固定 uid/gid，避免依赖镜像默认）。
USER 65532:65532

# 默认启动守护进程；compose / k8s 可用 command 覆盖（--data-dir/--bootstrap 等）。
ENTRYPOINT ["gsn-daemon"]
CMD ["--data-dir", "/data", "--listen", "0.0.0.0", "--port", "4001", "--api-port", "4002"]

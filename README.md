<p align="center">
  <img src="frontend/public/aether_adaptive.svg" width="120" height="120" alt="Aether Logo">
</p>

<h1 align="center">Aether</h1>

<p align="center">
  <strong>一站式 AI 基础设施平台</strong><br>
  支持 Claude / OpenAI / Gemini 及其 CLI 客户端的统一接入、格式转换、正/反向代理, 致力于成为用户驱动AI服务的底座
</p>
<p align="center">
  <a href="#简介">简介</a> •
  <a href="#部署">部署</a> •
  <a href="#api-文档">API 文档</a> •
  <a href="#环境变量">环境变量</a> •
  <a href="#qa">Q&A</a>
</p>


---

## 简介

Aether 是一个自托管的 AI API 网关，为团队和个人提供多租户管理、智能负载均衡、成本配额控制和健康监控能力。通过统一的 API 入口，可以无缝对接 Claude、OpenAI、Gemini 等主流 AI 服务及其 CLI 工具。

<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="docs/architecture/architecture-dark.svg">
    <source media="(prefers-color-scheme: light)" srcset="docs/architecture/architecture-light.svg">
    <img src="docs/architecture/architecture-light.svg" width="680" alt="Aether Architecture">
  </picture>
</p>

页面预览: https://fawney19.github.io/Aether/

## 部署

### Docker Compose（推荐：预构建镜像）

```bash
# 1. 克隆代码
git clone https://github.com/fawney19/Aether.git
cd Aether

# 2. 配置环境变量
cp .env.example .env
# .env 包含数据库、JWT 和数据加密密钥，先限制为仅当前用户可读写
chmod 600 .env
# 生成 JWT / 加密 / Postgres / Redis 独立随机密钥，并填入 .env
./generate_keys.sh
# 编辑 .env 设置 ADMIN_PASSWORD

# 3. Docker 部署 / 更新（PostgreSQL + Redis）
docker compose pull && docker compose up -d
```

### Docker Compose（本地构建镜像）

```bash
# 1. 克隆代码
git clone https://github.com/fawney19/Aether.git
cd Aether

# 2. 配置环境变量
cp .env.example .env
./generate_keys.sh  # 生成 JWT_SECRET_KEY / ENCRYPTION_KEY 等独立密钥，并填入 .env
# 编辑 .env 设置 ADMIN_PASSWORD

# 3. 构建 / 更新镜像（仅构建，不启动容器）
git pull
./deploy.sh
# 可选：额外打自定义 tag（同时保留 aether-app:latest）
# ./deploy.sh --tag v20260427

# 4. 启动容器（自动执行数据库迁移）
docker compose -f docker-compose.build.yml up -d --no-build
```

`./deploy.sh` 构建脚本参数与环境变量：

- `--tag, -t <tag>`：在保留 `aether-app:latest` 的同时额外打一个自定义 tag（例如 `--tag v20260427`）。
- `--force, -f`：跳过 `.code-hash` 检查，强制重建镜像。
- `--tunnel-mode <mode>` / `AETHER_TUNNEL_MODE`：`aether-tunnel` 打包方式：
  - `source`（默认）：从当前源码构建
  - `release`：下载上游 Release 二进制（需搭配 `--tunnel-release-tag` / `AETHER_TUNNEL_RELEASE_TAG`，如 `tunnel-v0.3.17`）
  - `none`：不打包 tunnel
- `--tunnel-release-tag <tag>` / `AETHER_TUNNEL_RELEASE_TAG`：release 模式下载的上游 tag（例如 `tunnel-v0.3.17`）。
- `--tunnel-release-repo <repo>` / `AETHER_TUNNEL_RELEASE_REPO`：release 模式下载仓库，默认 `fawney19/Aether`。
- `--build-cache` / `DOCKER_BUILD_CACHE`：是否启用 Docker BuildKit 本地缓存导入/导出，默认 `0`；设为 `1` 或传入 `--build-cache` 启用。
- `--no-build-cache`：禁用 Docker BuildKit 本地缓存。
- `--docker-no-cache, --no-cache` / `DOCKER_NO_CACHE`：禁用 Docker 层缓存并强制重建（向 Docker 传递 `--no-cache`）。
- `--build-cache-dir <dir>` / `DOCKER_BUILD_CACHE_DIR`：Docker BuildKit 本地缓存目录，默认 `.docker-cache/app`。
- `LOCAL_APP_IMAGE`：本地构建镜像名，默认 `aether-app:latest`。
- `AETHER_BUILD_VERSION`：应用显示版本，默认根据 git describe 自动推导。

应用镜像默认以固定非 root 身份 `65532:65532` 运行；Compose 同时移除全部 Linux capabilities、禁止提权、启用只读根文件系统，并仅提供带 `nosuid,nodev,noexec` 的 `/tmp` 临时文件系统。若宿主机不适合使用固定 UID/GID，可在 `.env` 中把 `AETHER_CONTAINER_UID` / `AETHER_CONTAINER_GID` 改成其他非零数字身份。`install.sh` 会按安装用户自动推导这些值。

### 一键更新

Docker Compose 部署后，可在部署目录直接执行：

```bash
./update.sh
```

`update.sh` 会拉取最新 `app` 镜像并重建 `app` 容器，Docker named volumes 和 `./logs` 不会被删除。

仓库自带的 Docker Compose 默认把应用日志输出到容器 `stdout/stderr`，直接用 `docker compose logs -f app` 查看，并由 Docker 轮转日志，避免非 root 用户被宿主机日志目录权限拖垮启动。如果你确实需要文件日志，需要在 compose 里把 `AETHER_LOG_DESTINATION` 改成 `file|both`，额外挂载目录到 `/opt/aether/logs`，并让它归 `.env` 中配置的容器 UID/GID 所有；只读根文件系统不会阻止显式可写挂载。

管理后台右上角“版本信息”会检测新版本。Docker Compose 部署只提示版本，实际更新继续执行 `./update.sh`；systemd / launchd / 二进制部署才使用后台自更新，流程是下载对应平台的 GitHub Release 包、强制校验 `SHA256SUMS`、解压到 `/opt/aether/releases/<version>`，再切换 `/opt/aether/current` 并退出进程，交给 systemd / launchd 拉起新版本。

正式 Release 还会发布由 GitHub Actions OIDC / Sigstore 签发的 SLSA build provenance。需要验证发布者身份时，下载目标 tarball 和 `AETHER_RELEASE_PROVENANCE.sigstore.json`，并把 `TAG` 设置为对应 Release tag：

```bash
gh attestation verify "aether-${TAG}-linux-amd64.tar.gz" \
  --repo fawney19/Aether \
  --signer-workflow fawney19/Aether/.github/workflows/release.yml \
  --source-ref "refs/tags/${TAG}" \
  --bundle AETHER_RELEASE_PROVENANCE.sigstore.json
```

`docker-compose.yml` 中的官方 PostgreSQL 与 Redis 镜像均固定到多架构 OCI index digest。升级这些依赖时应在发布变更中显式更新 digest，避免同名 tag 在无人审查的情况下改变部署内容。

正式发布到 GHCR 和 Docker Hub 的多架构 Aether 镜像也带有同一 GitHub Actions OIDC / Sigstore provenance；生产 `Dockerfile.app` 的 BusyBox 与 Distroless 基础镜像同样固定到多架构 OCI index digest。

源码或本地构建版本不会启用后台在线更新，请继续使用源码更新流程。Docker Compose 用户如果希望“容器重建后也保持镜像层面的新版本”，仍建议定期运行 `./update.sh` 拉取并重建 app 镜像。服务器访问 GitHub 需要代理时，可设置 `AETHER_UPDATE_PROXY_URL`，也兼容 `UPDATE_PROXY_URL`、`HTTPS_PROXY`、`ALL_PROXY`、`HTTP_PROXY` 以及 `NO_PROXY`。共享出口触发 GitHub API 限流时，可设置只读 `AETHER_UPDATE_GITHUB_TOKEN`，也兼容 `GITHUB_TOKEN` / `GH_TOKEN`。下载总超时默认 600 秒，连续无响应/无数据默认 30 秒，可通过 `AETHER_UPDATE_DOWNLOAD_TIMEOUT_SECS` 和 `AETHER_UPDATE_DOWNLOAD_IDLE_TIMEOUT_SECS` 调整。

标准 Docker Compose 使用 Docker named volumes 存放 Postgres/Redis 数据。

如果是本地源码构建镜像的部署，继续使用：

```bash
./deploy.sh
```

如果要在本机联调“管理后台在线更新”本身，可启动仓库内置的 release-layout 测试环境：

```bash
docker compose -f docker-compose.release-local.yml up -d --build
```

这套环境会用当前源码构建一个本地测试镜像，但编译为 `release` 类型，并默认伪装成 `v0.7.0`，这样后台会按正式发布版逻辑开放“立即更新”。默认监听 `http://127.0.0.1:18085`，数据目录使用 `./data-release-local`；日志默认走 `docker logs`，不会影响你正在跑的源码构建容器。

如果这套容器在 `prepare-update` 时访问 GitHub 失败，而你本机是通过代理出网，请在 `.env` 里把 `AETHER_UPDATE_PROXY_URL` 写成宿主机地址，例如 `http://host.docker.internal:7890`；容器内的 `127.0.0.1` 指向容器自身，不是宿主机。

如果想重置这套联调环境（包括 `/opt/aether/current` 和已下载的历史版本），执行：

```bash
docker compose -f docker-compose.release-local.yml down -v
```

可选变量：

- `AETHER_RELEASE_LOCAL_VERSION`：本地联调镜像对外声明的当前版本，默认 `v0.7.0`
- `AETHER_RELEASE_LOCAL_PORT`：本地联调端口，默认 `18085`
- `LOCAL_RELEASE_APP_IMAGE`：本地联调镜像名，默认 `aether-app:release-local`

### 一键安装（PostgreSQL + Redis）

```bash
git clone https://github.com/fawney19/Aether.git
cd Aether
curl -fsSL https://raw.githubusercontent.com/fawney19/Aether/main/install.sh | sudo bash -s -- --mode compose
```

正式版和 Nightly 自动构建仅提供 Linux `amd64` / `arm64` 二进制包，Docker 镜像同样支持这两种架构。macOS 用户可使用 Docker 或自行从源码构建；安装脚本保留对历史 macOS 制品的兼容。独立 Aether Tunnel 的多平台发行不受此调整影响。

原生 Linux systemd 安装需先准备 PostgreSQL，将连接串通过 `DATABASE_URL` 传给安装进程，并选择 `--mode single-node`；不再自动创建本地数据库文件。

### Nightly（每日 main 构建）

Nightly workflow 每天从 `main` 的固定 commit 构建并发布滚动的 GitHub Release `nightly`，同时推送多架构 GHCR 镜像 `ghcr.io/fawney19/aether:nightly`。Nightly 是预发布版本，适合验证最新代码，不保证与正式版相同的稳定性。滚动 Release 需要仓库保持关闭 GitHub Release immutability。

安装最新 nightly（PostgreSQL + Redis）：

```bash
curl -fsSL https://raw.githubusercontent.com/fawney19/Aether/main/install.sh | sudo bash -s -- --mode compose --channel nightly
```

Docker Compose 用户可在部署目录的 `.env` 中设置 `APP_IMAGE=ghcr.io/fawney19/aether:nightly`，然后运行 `./update.sh` 获取下一次 nightly。二进制部署请沿用已有 PostgreSQL 环境配置，并使用 `--mode single-node --channel nightly` 重新运行安装脚本升级；当前管理后台的在线更新列表只跟踪正式版/RC/Beta，不会自动提示下一次 nightly。

## 本地开发

依赖 Docker、Rust toolchain、Node.js 和 make。
首次启动前需要在 `.env` 中设置 `ADMIN_PASSWORD`，用于创建本地管理员。

```bash
make dev
```

`make dev` 会同时启动后端 `aether-gateway` 和前端 `frontend` 的 Vite dev server。需要单独启动时可使用 `make dev-backend` 或 `make dev-frontend`。
Postgres / Redis 本地依赖未就绪时，`make dev` 会自动执行 `docker compose up -d postgres redis`。
`make dev` 会先完成后端编译，再开始计算服务健康检查超时。数据库 schema 和必要的派生数据准备也会在启动时自动完成；通常不需要手动区分 migration 与 backfill。升级不会主动重写或清除已有业务历史记录，新写入会直接遵循当前的数据持久化策略。排查或部署前预执行时可使用：

```bash
make db-status
make db-prepare
```

如需手动执行迁移、回填或分开启动，也可以使用下面的命令：

```bash
# 启动依赖
docker compose -f docker-compose.build.yml up -d postgres redis

# 数据库迁移（仅在已有数据库引入新 migration 时需要）
./dev.sh --migrate

# 数据回填
./dev.sh --apply-backfills

# 后端
./dev.sh

# 前端
cd frontend && npm install && npm run dev
```

`./dev.sh` 现在只保留一种本地模式：

| 角色 | 本地地址 | 说明 |
|------|----------|------|
| Rust gateway（默认 background） | 默认 `http://localhost:8084` | `aether-gateway`，本地唯一公开入口；实际端口由 `APP_PORT` 控制 |

本地默认链路是：

```text
client -> rust gateway (aether-gateway) -> execution_runtime/provider transport
```

其中：

- `aether-gateway` 负责公开入口、健康检查、格式转换、本地执行 runtime，以及当前已迁到 Rust 的 frontdoor/control/background 路径。
- `./dev.sh` 不再启动 Python 宿主；未下沉到 Rust 的 legacy 路由会直接失败。
- `./dev.sh --migrate` 会复用 `.env` 里的数据库配置，显式执行一次数据库迁移后退出。
- `./dev.sh` 默认把 `AETHER_GATEWAY_VIDEO_TASK_TRUTH_SOURCE_MODE` 设为 `rust-authoritative`，避免本地还依赖 Python sync report 语义。
- 空库首次启动会自动初始化到当前 baseline。
- `aether-gateway` 默认启动不会自动应用后续 schema migration；如果数据库版本落后，服务会拒绝启动，并提示先执行 `aether-gateway --migrate`。
- 仓库自带的 `docker-compose.yml` 和 `docker-compose.build.yml` 都已把 `AETHER_GATEWAY_AUTO_PREPARE_DATABASE` 设为默认开启，因此无论是预构建镜像部署，还是先 `./deploy.sh` 构建本地镜像再执行 `docker compose -f docker-compose.build.yml up -d --no-build`，常规启动都会在监听端口前自动执行挂起的 migration 和 backfill。
- `./deploy.sh --tag <tag>` 会在保留 `aether-app:latest` 的同时额外打一个 `aether-app:<tag>`，方便手工发布或留档；`docker-compose.build.yml` 默认仍使用 `aether-app:latest`。

## Codex 远程协同

`aether-vscodex/` 是独立的 VS Code Codex 协同模块：同步模式跟随 VS Code 官方 Codex 面板当前会话且不另起进程；异步模式使用独立 app-server，让浏览器自行列出、恢复、新建和切换会话。两种模式都能从本机 URL 或 Aether 云端查看输出、发送消息和处理授权，模块内的 Vue 前端提供中英文界面。

安装、云端配对和安全边界请参阅 [`aether-vscodex/README.md`](aether-vscodex/README.md)。

## Aether Tunnel (可选)

Aether Tunnel 是配套的正向代理节点，部署在海外 VPS 上，为墙内的 Aether 实例中转 API 流量。

- Docker Compose 部署或下载预编译二进制直接运行
- 提供 macOS/Linux 与 Windows 一键脚本，自动下载最新 `tunnel-v*` 制品并向现有 `aether-tunnel.toml` 追加 `[[servers]]`
- 通过 `aether-tunnel setup` 完成交互式配置，自动注册为系统服务
- 详细文档见 [apps/aether-tunnel/README.md](apps/aether-tunnel/README.md)

## API 文档

- Embeddings: [OpenAI compatible `POST /v1/embeddings`](docs/api/embeddings.md)
- Rerank: [OpenAI/Jina compatible `POST /v1/rerank`](docs/api/rerank.md)
- Responses WebSocket mode: [protocol and Aether behavior](docs/WebSocket-Mode.md)
- WebSocket probes: [Codex](docs/operations/codex-responses-websocket-probe.md) · [OpenAI Responses](docs/operations/openai-responses-websocket-probe.md)

## 环境变量

- `APP_PORT`：`aether-gateway` 唯一监听端口，固定绑定 `0.0.0.0:${APP_PORT}`
- `AETHER_GATEWAY_DEPLOYMENT_TOPOLOGY`：部署拓扑，默认 `multi-node`；单节点配置显式设为 `single-node`
- `AETHER_GATEWAY_NODE_ROLE`：节点角色，默认 `background`；多节点可设为 `frontdoor`，单节点可设为 `all`
- `DATABASE_URL`：PostgreSQL 连接串，例如 `postgresql://USER:PASSWORD@HOST:5432/aether`
- `AETHER_GATEWAY_DATA_POSTGRES_MIN_CONNECTIONS` / `AETHER_GATEWAY_DATA_POSTGRES_MAX_CONNECTIONS`：数据库连接池手动覆盖值；未配置时 PostgreSQL 按每核 `4` 条自动推导，总池范围为 `32-100`。该预算按进程计算，多实例部署应按数据库连接上限显式分配
- `AETHER_GATEWAY_DATA_POSTGRES_STATEMENT_TIMEOUT_MS` / `AETHER_GATEWAY_DATA_POSTGRES_LOCK_TIMEOUT_MS`：普通数据库连接的单条 SQL / 锁等待期限，默认 `30000` / `3000` 毫秒，显式 `0` 关闭；不是整个事务总期限。迁移与历史 backfill 使用独立连接放宽，事务可通过局部设置覆盖
- `AETHER_USAGE_EVENT_CAPTURE_MEMORY_BUDGET_BYTES`：usage 诊断正文共享预算，默认 `134217728`（128 MiB），按 JSON 堆内存估算，覆盖进入终态队列的 seed、Redis 解码后的事件、数据库写入 DTO 及其正文副本。额度不足或显式 `0` 时先保留计费事实，再舍弃诊断正文；已有清空或禁用状态保持不变，其余标记截断。预算随正文保留到释放，后台构建或压缩不会因调用方取消而提前归还额度。该额度不覆盖原始 Redis 批次、解码临时分配、序列化及压缩结果、协议观察缓冲或进程总内存；可通过 `usage_runtime_event_capture_memory_*` 指标观察
- `AETHER_GATEWAY_USAGE_QUEUE_PAYLOAD_MAX_BYTES`：新增 usage 队列消息的完整 JSON payload 上限，默认 `1048576`（1 MiB），按序列化后的 UTF-8 字节计算，显式 `0` 非法。超限先保留计费事实并舍弃诊断字段；仍超限或无法保留计费语义时拒绝入队，终态消息尝试受限数据库落库，失败则明确失败，不继续 Redis 重试。该限制不覆盖存量 Redis 消息、整个读取批次、DLQ 或进程总内存。`usage_runtime_queue_payload_*` 导出上限及进程级降级、拒绝编码尝试次数，包含入队和重试预校验，不代表唯一事件数；`usage_runtime_enqueue_retry_permanent_failure_total` 记录永久输入错误导致的重试拒绝或终止
- `AETHER_USAGE_QUEUE_READ_PAYLOAD_BUDGET_BYTES` / `AETHER_USAGE_QUEUE_READ_BATCH_PAYLOAD_BYTES`：usage worker 读取和重领共用的进程级逻辑 payload 预留，默认总额 `134217728`（128 MiB）、单批目标 `8388608`（8 MiB）。按当前 `QUEUE_PAYLOAD_MAX_BYTES` 推导实际 COUNT，默认最多读取 8 条，自动扩容使用实际 COUNT 判断批次是否读满。预留覆盖读取、整批处理和确认，额度不足等待；取消/失败释放。单批目标至少允许一条，当前 payload 上限大于总额时读取报配置错误。`0` 或非法值回退默认，过大值收敛到约 4 GiB 的有效总额。收到消息后按全部字段值长度缩减多余预留；历史消息、其他生产者使用更高上限或额外字段可能超出估算，仍继续原计费流程并记录 `usage_runtime_queue_read_oversized_*`。`usage_runtime_queue_read_*` 同时导出预留、等待与累计字段字节；该预留不是 RESP 解码、连接缓冲容量、字段结构、诊断 JSON、DLQ 或进程 RSS 的硬上限，旧公开 Vec 读取接口不携带处理阶段预留
- `AETHER_USAGE_DLQ_ENCODING_BUDGET_BYTES` / `AETHER_USAGE_DLQ_ENCODING_MAX_JOBS`：死信原文和 JSON 编码独立共享预留，默认 `67108864`（64 MiB）、最多 `4` 个后台编码及写入任务。根据原始字段、ID、错误字符串及 JSON 最坏 6 倍转义一次预留；预算占满或单条超总额时立即失败，worker 保留原消息等待重领，不截断账务原文。编码失败会继续处理同批其他消息，只确认成功项，批次末尾仍报告失败；存储转移失败则停止该批后续处理。取消编码等待不会提前归还仍在后台使用的额度。`0`/非法值回退默认，bytes 最大约 4 GiB，jobs 最大 128；超大存量消息可能需要调高总额后恢复。`usage_runtime_dlq_encoding_*` 导出额度、在途任务、拒绝和编码尝试次数；不包含字段结构、字符串额外容量、Redis 命令/连接副本或进程 RSS。内置 Redis/Memory worker 将死信追加、源 ACK 和删除作为一次原子转移，同一源 stream、消费组及 pending ID 的并发或重试只追加一次；Redis 要求 7+ 及 `EVAL/TYPE/XPENDING/XADD/XACK/XDEL` 权限，Cluster 两键须同 slot（当前默认键不自动迁移）。源和 DLQ 不能同名。源已不在 PEL 时不宣称已归档；外部 ACK/trim/delete 及多消费组仍有原来的删除语义。公开 `push_dead_letter` 仍为追加接口，未实现新原子 trait 方法的外部后端沿用追加后 ACK，仍可能重复归档
- `AETHER_GATEWAY_MAX_IN_FLIGHT_REQUESTS`：单实例请求并发上限；未配置时按 CPU 自动推导（基础范围 `512-65536`），低文件描述符预算时会进一步下调
- `AETHER_GATEWAY_MAX_HTTP_CONNECTIONS`：二进制入口全部监听分片共用的入站 TCP 连接上限，包含握手、空闲 keep-alive 和 HTTP 升级后仍存活的 socket。未设置或 `0` 时使用请求上限与 WebSocket 上限之和；自动及显式值均最多 `65536`，已知 FD soft limit 时进一步限制为 `max(1, (FD - 256) / 2)`。接入后立即尝试取得额度，满额时关闭新连接，不创建 HTTP 处理任务、不等待额度，不返回 HTTP 状态码；取消、解析失败和连接释放归还，WebSocket 升级不会提前归还。HTTP/2 多流共用一个 TCP 许可，原请求和 WebSocket 准入仍独立有效。`gateway_http_connections_*` 导出配置上限、当前数、高水位、拒绝数及 accept 错误数。该限制不包含 kernel backlog、上游、Redis 或数据库连接，也不是整个进程 FD/内存硬上限。临时 accept 错误重试，资源类错误退避一秒后重试，避免单次错误停止监听
- `AETHER_GATEWAY_REQUEST_BODY_BUFFER_BUDGET_MB`：单实例同时读取和解压请求体的加权内存预算，默认 `256MB`；压缩和未知长度上传按实际缓冲增长申请额度，解压时计入同时存活的输入和输出。额度不足返回 `503`；接近单请求上限的压缩上传需要为输入和解压输出预留额外预算
- `AETHER_GATEWAY_REQUEST_BODY_READ_TIMEOUT_MS`：请求体完整读取超时，默认 `120000ms`；显式设为 `0` 时关闭，非零值限制在 `1000-600000ms`
- `AETHER_GATEWAY_UPSTREAM_STREAM_IDLE_TIMEOUT_MS`：上游流首包后的空闲超时，默认 `300000ms`；请求执行配置中的 `read_ms` 优先，显式 `0` 关闭对应超时。网关生成的 keepalive 不会重置计时
- `AETHER_GATEWAY_STREAM_CAPTURE_MEMORY_BUDGET_BYTES`：进程内流式响应诊断捕获的共享字节预算，默认 `134217728`（128 MiB）；包含 provider/client 捕获容量和扩容时的新旧分配。额度不足时仅截断审计副本，显式 `0` 关闭此类捕获；协议解析、客户端传输和计费观察继续执行。该预算不包含协议解析缓冲、终态编码及 usage 队列副本，不是进程总内存上限
- `AETHER_MAX_REQUEST_BODY_MB`：单请求解压后请求体上限，默认 `256MB`；显式设为 `0` 表示不再收紧默认值，但仍受 `256MB` 安全硬上限约束
- `AETHER_MAX_INTERNAL_BUFFERED_BODY_MB`：heartbeat、管理探测等内部整包响应体上限，默认 `64MB`；显式设为 `0` 表示不再收紧默认值，但仍受 `256MB` 安全硬上限约束
- `AETHER_TUNNEL_NODE_STATUS_QUEUE_CAPACITY`：隧道节点状态上报队列容量，默认 `1024`；满载时拒绝新事件，避免控制面故障导致无界内存增长
- `AETHER_TUNNEL_RELAY_ALLOW_PRIVATE_TARGETS`：跨网关 owner relay 解析到私有/保留地址时的显式运维开关，默认关闭；仅当多网关 relay URL 是受控的内网 HTTPS 地址时设置为 `true`。它不改变普通 provider 请求的 DNS/代理策略，也不允许明文 HTTP 非 loopback relay
- `AETHER_TUNNEL_RELAY_PRIVATE_HOST_ALLOWLIST`：更窄的 owner relay 私网例外，填写逗号分隔的精确主机名（例如 `gateway-a.internal,gateway-b.internal`，忽略大小写和末尾点）；仅这些主机解析出的私有地址会被允许，并且请求仍使用解析后地址 pin。不要填写通配符或 `.internal` 这类后缀
- `AETHER_INTERNAL_GATEWAY_AUTH_SECRET`：旧版 `/api/internal/gateway/*` 高权限控制面的独立 HMAC 密钥，至少 `32` 字节；未配置时该控制面返回 `404`。不要复用 JWT、数据加密或 tunnel relay 密钥，多节点必须使用同一值及共享 Redis 防重放
- `AETHER_GATEWAY_SECURITY_CACHE_TTL_MS`：IP 黑白名单本地缓存时间，默认 `1000ms`，写操作会主动失效相关缓存
- `AETHER_MAX_REDACTED_SYNC_RESPONSE_BODY_MB`：PII 恢复同步响应缓冲上限，默认 `64MB`；显式设为 `0` 表示不再收紧默认值，但仍受 `256MB` 安全硬上限约束
- `REDIS_URL`：Redis 连接串；仅 Postgres + Redis 的 Docker Compose 部署需要配置
- `AETHER_RUNTIME_BACKEND=memory|redis`：运行时缓存/协调后端。配置 Redis 时使用 `redis`，否则使用 `memory`；多节点部署和需要跨 gateway 重启恢复 OpenAI Responses continuation history 的部署必须使用共享 Redis
- `AETHER_GATEWAY_DATABASE_MODE=auto|verify-only`：数据库启动策略，默认 `auto`，自动完成挂起的 schema migration 和 backfill；`verify-only` 仅检查并在数据库落后时拒绝启动
- `AETHER_GATEWAY_AUTO_PREPARE_DATABASE`：旧版兼容开关；新配置请使用 `AETHER_GATEWAY_DATABASE_MODE`
- `JWT_SECRET_KEY` / `ENCRYPTION_KEY`：认证和敏感数据加密所需密钥
- `AETHER_BACKUP_ENCRYPTION_KEY`：推荐的 S3 备份独立加密密钥；缺省回退到 `ENCRYPTION_KEY`。新备份使用带 key ID 的 AES-256-GCM v2 envelope，轮换前必须保留旧密钥
- `API_KEY_PREFIX`：用户和管理员新建 API Key 时使用的前缀，默认 `sk`
- `ADMIN_USERNAME` / `ADMIN_PASSWORD` / `ADMIN_EMAIL`：首次启动时自举首个本地管理员；`install.sh` 会提示输入管理员密码
- `CORS_ORIGINS` / `CORS_ALLOW_CREDENTIALS`：前端跨域来源控制；如果要跨域带登录 Cookie，`CORS_ORIGINS` 不能写 `*`
- `RUST_LOG`：Rust 日志过滤，例如 `aether_gateway=info`、`aether_gateway=debug,sqlx=warn`
- `DB_PASSWORD` / `REDIS_PASSWORD`：Docker Compose 后端密码，首次安装时分别随机生成；手工部署必须替换示例占位值，不要互相复用

运行日志由独立后台线程写入 stdout 和文件，每个输出队列最多 4096 条、保留正文最多 8 MiB（包含正在写入的记录），单条最多 256 KiB。队列满、正文预算不足或单条超限时整条丢弃，不等待日志设备；`Both` 两个输出独立降级。`logging_stdout_*` 和 `logging_file_*` 指标记录丢弃和写入错误，网关指标沿用其命名空间前缀。正常退出时日志最多等待 2 秒排空；这不是请求优雅排空或整个进程退出期限。日志格式化仍在调用线程执行，日志预算不包含格式化临时内存，运行日志也不能作为可靠计费账本。

### S3 备份离线恢复

先从 S3 下载完整的 `.json.zst.aes256gcm` 对象，再使用原始的完整 S3 object key 做认证解密。恢复工具只验证并输出本地 JSON，不会直接写数据库；数据库导入仍应在维护窗口通过管理端完成。

```bash
AETHER_BACKUP_ENCRYPTION_KEY='原备份密钥' \
  cargo run -p aether-gateway --bin aether-backup-restore -- \
  --input ./backup.json.zst.aes256gcm \
  --object-key 'aether/backups/aether-data-backup-20260822-010000.json.zst.aes256gcm' \
  --output ./restored-backup.json
```

工具默认拒绝覆盖，输出采用原子写并在 Unix 上设置为 `0600`；Unix 可用 `--overwrite` 原子替换，Windows 为避免非原子删除窗口会要求选择新输出路径。密钥不能作为命令行参数。可使用 `AETHER_BACKUP_ENCRYPTION_KEY`、兼容用 `AETHER_GATEWAY_DATA_ENCRYPTION_KEY` / `ENCRYPTION_KEY`、受保护的 `--key-file`，或 `AETHER_BACKUP_KEYRING_FILE`。Keyring JSON 格式为 `{"version":1,"keys":["当前或历史 v2 secret"],"legacy_v1":["旧 v1 secret"]}`；条目也可写成 `{"secret":"..."}`（兼容字段名 `key`）。也可由 `AETHER_BACKUP_HISTORICAL_KEYS_JSON` 提供同一结构。密钥文件必须是非符号链接的普通文件，Unix 下权限需为 `0600` 或更严格。

默认限制密文为 `512MiB`、解压后 JSON 为 `1GiB`，可通过受限的 `--max-encrypted-mib` / `--max-json-mib` 调整。网关最多扫描同一备份前缀下 10,000 个对象，并且不会自动删除 S3 对象：`backup_s3_retention_count` 只用于报告超出保留数量的清理候选。旧明文备份在创建并验证加密副本后仍会保留，必须通过 bucket lifecycle 或支持版本条件的外部清理工具移除；启用 Versioning 时还需清理 noncurrent versions，Object Lock/retention 可能阻止物理删除。

---

## 许可证

本项目采用 [Aether 非商业开源许可证](LICENSE)。允许个人学习、教育研究、非盈利组织及企业内部非盈利性质的使用；禁止用于盈利目的。商业使用请联系获取商业许可。

## 联系作者

<p align="center">
  <img src="docs/author/qq_qrcode.jpg" width="200" alt="QQ二维码">
  &nbsp;&nbsp;&nbsp;&nbsp;
  <img src="docs/author/qrcode_1770574997172.jpg" width="200" alt="QQ群二维码">
</p>

## Star History

[![Star History Chart](https://api.star-history.com/svg?repos=fawney19/Aether&type=date&legend=top-left)](https://www.star-history.com/?repos=fawney19%2FAether&type=date&legend=top-left)

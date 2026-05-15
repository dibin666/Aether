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
./generate_keys.sh  # 生成 JWT_SECRET_KEY / ENCRYPTION_KEY, 并填入 .env
# 编辑 .env 设置 ADMIN_PASSWORD

# 3. 首次部署 / 更新
docker compose pull && docker compose up -d

# 4. 默认会在 app 启动前自动执行挂起的 migration / backfill
#    如需手工控制，可在 .env 中设 AETHER_GATEWAY_AUTO_PREPARE_DATABASE=false
#    然后按需执行：
docker compose run --rm app --migrate
docker compose run --rm app --apply-backfills

# 5. 升级前备份 (可选)
docker compose exec postgres pg_dump -U postgres aether | gzip > backup_$(date +%Y%m%d_%H%M%S).sql.gz
```

### Docker Compose（SQLite）

```bash
# 1. 克隆代码
git clone https://github.com/fawney19/Aether.git
cd Aether

# 2. 配置环境变量
cp .env.example .env
./generate_keys.sh  # 生成 JWT_SECRET_KEY / ENCRYPTION_KEY, 并填入 .env
# 编辑 .env 设置 ADMIN_PASSWORD

# 3. 首次部署 / 更新
docker compose -f docker-compose.sqlite.yml pull && docker compose -f docker-compose.sqlite.yml up -d

# 4. 升级前备份（可选）
cp -a data/aether.db backup_$(date +%Y%m%d_%H%M%S).db
```

### Docker Compose（本地构建镜像）

```bash
# 1. 克隆代码
git clone https://github.com/fawney19/Aether.git
cd Aether

# 2. 配置环境变量
cp .env.example .env
./generate_keys.sh  # 生成 JWT_SECRET_KEY / ENCRYPTION_KEY, 并填入 .env
# 编辑 .env 设置 ADMIN_PASSWORD

# 3. 构建 / 更新镜像（仅构建，不启动容器）
git pull
./deploy.sh
# 可选：额外打自定义 tag（同时保留 aether-app:latest）
# ./deploy.sh --tag v20260427

# 4. 启动容器（自动执行数据库迁移）
docker compose -f docker-compose.build.yml up -d --no-build
```

### 一键安装（可选部署方式 Linux: systemd; Mac: launchd）

```bash
cd Aether && cd Aether
curl -fsSL https://raw.githubusercontent.com/fawney19/Aether/main/install.sh | sudo bash
```

运行后按提示输入语言、版本和部署方式。固定安装某个 tag 时，版本选择选 `2`，再输入类似 `v0.7.0-rc23` 的 tag。默认会安装最新预发布版本；Docker Compose 模式默认使用 `pre` 镜像通道。
二进制安装在下载 Release 压缩包前会询问是否使用下载加速源；选择使用时会先打印原始 GitHub URL，再要求输入新的压缩包下载 URL。非交互式安装可用 `AETHER_RELEASE_ARCHIVE_URL` 指定压缩包 URL。
如果安装目录里已经有配置，脚本会优先复用：Docker Compose 保留已有 `.env`，二进制服务模式保留已有 `/etc/aether/aether-gateway.env`。只有首次生成新配置时才会提示输入管理员密码。

```text
请选择安装语言 / Choose installer language:
  1) 中文
  2) 英语 / English

请输入选项 / Enter choice [1]:

请选择 Aether 版本:
  1) 最新预发布版本
  2) 指定 tag，例如 v0.7.0-rc23

请输入选项 [1]:

请选择 Aether 部署模式:
  1) Docker Compose: 应用 + Postgres + Redis
  2) 单机服务: systemd/launchd + SQLite + 进程内运行时
  3) 集群节点服务: systemd/launchd + 共享数据库 + Redis
  4) Docker Compose: 应用 + SQLite

请输入选项 [2]:

是否使用下载加速源?
  1) 否，使用原始 GitHub 地址
  2) 是，手动填写新的下载 URL

请输入选项 [1]:
```

安装后的常用命令（Linux systemd）：

```bash
sudo systemctl status aether-gateway --no-pager
sudo journalctl -u aether-gateway -f
sudo systemctl restart aether-gateway
```

安装后的常用命令（macOS launchd）：

```bash
sudo launchctl print system/com.aether.gateway
sudo launchctl kickstart -k system/com.aether.gateway
sudo launchctl bootout system /Library/LaunchDaemons/com.aether.gateway.plist
tail -f /var/log/aether/aether-gateway.out.log /var/log/aether/aether-gateway.err.log
```

macOS 原生安装使用系统级 LaunchDaemon，默认以专用 `_aether` 服务账号运行；配置和密钥写入 `/etc/aether/aether-gateway.env`，数据和应用日志仍在 `/opt/aether`，launchd stdout/stderr 在 `/var/log/aether`。

默认单机数据和应用日志都在安装目录内：

```text
/opt/aether/data/aether.db
/opt/aether/logs
```

多节点不能使用 SQLite 或 `AETHER_RUNTIME_BACKEND=memory`。如果先只生成了多节点模板，编辑 `/etc/aether/aether-gateway.env` 后重跑安装脚本即可：

```env
AETHER_GATEWAY_DEPLOYMENT_TOPOLOGY=multi-node
AETHER_GATEWAY_NODE_ROLE=frontdoor
DATABASE_URL=postgresql://...
REDIS_URL=redis://...
```

### 本地开发

依赖 Docker、Rust toolchain、Node.js 和 make。

```bash
make dev
```

`make dev` 会同时启动后端 `aether-gateway` 和前端 `frontend` 的 Vite dev server。需要单独启动时可使用 `make dev-backend` 或 `make dev-frontend`。
Postgres / Redis 本地依赖未就绪时，`make dev` 会自动执行 `docker compose up -d postgres redis`。

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
| Rust frontdoor | 默认 `http://localhost:8084` | `aether-gateway`，本地唯一公开入口；实际端口由 `APP_PORT` 控制 |

本地默认链路是：

```text
client -> rust frontdoor (aether-gateway) -> execution_runtime/provider transport
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

## Aether Proxy (可选)

Aether Proxy 是配套的正向代理节点，部署在海外 VPS 上，为墙内的 Aether 实例中转 API 流量。

- Docker Compose 部署或下载预编译二进制直接运行
- 提供 macOS/Linux 与 Windows 一键脚本，自动下载最新 `proxy-v*` 制品并向现有 `aether-proxy.toml` 追加 `[[servers]]`
- 通过 `aether-proxy setup` 完成交互式配置，自动注册为系统服务
- 详细文档见 [apps/aether-proxy/README.md](apps/aether-proxy/README.md)

## API 文档

- Embeddings: [OpenAI compatible `POST /v1/embeddings`](docs/api/embeddings.md)
- Rerank: [OpenAI/Jina compatible `POST /v1/rerank`](docs/api/rerank.md)

## 环境变量

- `APP_PORT`：`aether-gateway` 唯一监听端口，固定绑定 `0.0.0.0:${APP_PORT}`
- `DATABASE_URL`：数据库连接串；SQLite 例如 `sqlite:///opt/aether/data/aether.db`，Postgres 例如 `postgresql://postgres:aether@postgres:5432/aether`
- `REDIS_URL`：Redis 连接串；仅 Postgres + Redis 的 Docker Compose 部署需要配置
- `AETHER_RUNTIME_BACKEND=memory|redis`：运行时缓存/协调后端。SQLite 默认用 `memory`，不会连接 Redis
- `AETHER_GATEWAY_AUTO_PREPARE_DATABASE`：常规启动前自动执行挂起的 schema migration 和 backfill；仓库自带的 `docker-compose.yml` 默认开启
- `JWT_SECRET_KEY` / `ENCRYPTION_KEY`：认证和敏感数据加密所需密钥
- `API_KEY_PREFIX`：用户和管理员新建 API Key 时使用的前缀，默认 `sk`
- `ADMIN_USERNAME` / `ADMIN_PASSWORD` / `ADMIN_EMAIL`：首次启动时自举首个本地管理员；`install.sh` 会提示输入管理员密码
- `CORS_ORIGINS` / `CORS_ALLOW_CREDENTIALS`：前端跨域来源控制；如果要跨域带登录 Cookie，`CORS_ORIGINS` 不能写 `*`
- `RUST_LOG`：Rust 日志过滤，例如 `aether_gateway=info`、`aether_gateway=debug,sqlx=warn`
- Docker Compose 的 `DB_PASSWORD` / `REDIS_PASSWORD` 默认使用 `aether`

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

[![Star History Chart](https://api.star-history.com/svg?repos=fawney19/Aether&type=Date)](https://star-history.com/#fawney19/Aether&Date)

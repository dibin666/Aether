# Fork 差异与上游合并冲突手册

本文件记录 `dibin666/Aether` 的 `rust` 分支相对 `fawney19/Aether` `main` 的持有行为。合并上游时先读本文件；它描述的是必须显式复核的功能契约，不代表冲突中可以整文件选择 `ours`。

## 强制更新纪律

每次上游合并都必须执行两次复核：合并前用于规划冲突，合并完成并通过验证后用于更新本文件。后一次不能省略，即使结论是“特有功能没有变化”。

合并后更新流程：

1. 先完成冲突解决、前后端验证并创建不可变的 merge commit。
2. 以该 merge commit 为 fork code baseline，重新比较 `upstream/main`，检查特有功能是否新增、改变、被上游吸收或意外丢失。
3. 更新快照日期、fork code baseline、upstream commit、merge-base、分叉计数、双边路径、当前待合入上游功能、功能清单、冲突规则和验证命令。
4. 在“最近一次合并后复核”中明确记录新增/改变/移除项；没有功能变化时也要写“无功能差异变化”，不能只改 commit hash。
5. 将本文件和 `SKILL.md` 的必要调整放进独立的文档 commit，不 amend merge commit。这样本文件可以稳定引用不会因文档修改而变化的 merge commit。
6. 最终交付必须同时报告 merge commit、文档更新 commit 和复核结论；两者完成前不得推送或宣布合并完成。

## 最近一次合并后复核

- 基线性质：初始 fork 差异盘点，尚非一次新的上游合并后复核。
- 结论：建立当前功能清单、24 个双边冲突路径和 20 个待合入上游提交的基线。
- 下次合并后：用实际 merge commit 替换本段，逐项记录“新增 / 改变 / 已被上游吸收 / 移除 / 无变化”。

## 基线快照

快照日期：2026-07-18（已执行 `git fetch --all --prune`）。

| 项目 | 值 |
|---|---|
| fork | `origin` → `git@github.com:dibin666/Aether.git` |
| upstream | `upstream` → `https://github.com/fawney19/Aether.git` |
| fork 分支 | `rust` |
| fork code baseline | `18789a454ec21ef723e1fa2af2b632d021e6d4fd`（当前初始盘点 HEAD；以后填写不可变 merge commit） |
| upstream HEAD | `6c33b8d8f` |
| merge-base | `3f5f65eb9a106d89808187ab01e050e30ad92f35` |
| 分叉计数 | fork-only 105，upstream-only 20 |
| fork 侧净改动 | 178 个路径，`+9874/-713` |
| upstream 侧净改动 | 147 个路径，`+15055/-2221` |
| 双边同时改动 | 24 个路径 |

合并前必须刷新这组数据；合并后再以已创建的 merge commit 重跑同一组比较并更新本文件：

```sh
git fetch --all --prune
git status --short --branch
git merge-base HEAD upstream/main
git rev-list --left-right --count HEAD...upstream/main
# fork 自 merge-base 起的行为
git diff --name-status upstream/main...HEAD
# upstream 自 merge-base 起的行为
git diff --name-status HEAD...upstream/main
# 两棵当前树的最终差异
git diff --name-status HEAD..upstream/main
```

三点区别：

- `upstream/main...HEAD` 用于识别 fork 持有的功能；
- `HEAD...upstream/main` 用于识别这次需要接入的上游功能；
- `HEAD..upstream/main` 用于确认合并前两棵树最终哪里不同。不要只看任意一种。

## 总体冲突策略

1. 双边改动文件默认选择 `manual hybrid`，禁止整文件 `ours`/`theirs`。
2. 优先采用上游的新数据模型、模块拆分和错误修复，再把下列 fork 行为重放到新结构中。
3. API、配置键、权限边界、任务键和用户可见页面是契约；纯重构形态不是契约。
4. 以当前净差异和行为测试为准。不要恢复已经被 revert 的旧调度实现。
5. 处理生成 SQL、导出列表和 trait 签名时，先合并真实契约，再传播到所有数据库适配器和测试替身。

## Fork 特有功能清单

### P0：OpenAI Audio Transcriptions 端到端支持

外部契约：

- 接受 `POST /v1/audio/transcriptions`，独立 API 格式为 `openai:transcription`；别名包括 `openai_transcription`、`transcription`、`transcriptions` 和路径本身。
- 请求必须是 `multipart/form-data`；必须有且只能有一个非空文本 `model`，至少一个非空文件字段 `file`；`stream` 缺省为 `false`，显式值只能是 `true`/`false`。
- multipart 解析器按借用切片解析二进制字段，不把音频误当 UTF-8，也不能把音频内容中的伪 boundary 当成分隔符。
- 模型映射只替换 multipart 中 `model` 字段，音频及其他字段字节保持不变。
- 同时支持同步和流式计划。二进制请求体通过 base64 计划字段传递，保留原始 `Content-Type` boundary；同格式 provider 使用直连鉴权、标准 URL 构造和 failover。
- 流式上游 SSE 原样透传；同步响应保留 JSON/text 等 response format。
- 管理端模型测试会生成 5 秒、16 kHz、单声道 WAV 并以 multipart 调用端点。
- usage 将请求类型记为 `audio`，支持转写 token usage，并把音频时长暴露为计费公式维度 `audio_duration_seconds`。
- 前端识别格式、缩写和别名；请求详情可展示转写文件名、媒体类型、大小、WAV 时长、模型、语言、提示词、响应文本和 segments 信息。

关键实现：

- 路由与入口：`apps/aether-gateway/src/control/route/ai.rs`、`control/auth/credentials.rs`、`handlers/public/ai_public.rs`、`handlers/proxy/finalize.rs`、`api/ai/openai.rs`、`api/ai/registry.rs`。
- multipart/格式：`crates/aether-ai/formats/src/formats/shared/multipart.rs`、`formats/openai/transcription.rs`、`formats/id.rs`、`formats/shared/routing.rs`、`formats/shared/passthrough.rs`。
- 计划与执行：`apps/aether-gateway/src/ai_serving/**`、`execution_runtime/**`、`executor/**`、`crates/aether-ai/serving/**`。
- provider 传输：`crates/aether-provider/transport/src/request_url/mod.rs`、`same_format_provider/mod.rs`、`conversion.rs`。
- usage/计费：`crates/aether-usage/runtime/src/{report,usage_mapper,write}.rs`、`crates/aether-billing/src/{default_rule,event_enrichment,pricing,service}.rs`。
- 管理与前端：`apps/aether-gateway/src/handlers/admin/provider/query/models/model_test*`、`crates/aether-admin/src/system.rs`、`frontend/src/api/endpoints/types/api-format.ts`、`frontend/src/features/usage/conversation/openai.ts`。
- 行为测试：`apps/aether-gateway/src/tests/ai_execute/{sync,stream}/transcription.rs`。

合并规则：

- 上游若重构 AI registry/planner，采用新结构，但上述格式、二进制保真、同步/流式和 usage/计费契约必须全部重接。
- `crates/aether-ai/formats/src/api.rs` 与 `apps/aether-gateway/src/ai_serving/pure/mod.rs` 当前还承载上游 Codex 缓存身份导出；必须同时保留上游导出和转写导出。
- 上游正在重构 processing-tier 计费和 usage body capture。以其新结算语义为主，再补回 `audio_duration_seconds`；不要用 fork 旧版 `pricing.rs`/`service.rs` 覆盖上游文件。

### P0：号池调度不变量

必须保留：

- cache-affinity 命中 pool group 时，把 rankable 的 provider/key/global-format priority 提升到最高优先级，避免软策略打散粘性。
- `pool_advanced.score_ranking_enabled` 控制是否执行高分候选阶段；默认 `true`，兼容旧别名 `pool_score_ranking_enabled`。关闭后直接进入分配模式/策略扫描，不读取 score candidate phase。
- 号池高级设置 UI 暴露“分数候选”开关并原样保存。
- `probing_enabled` 关闭时不显示虚假的热池目标、热池数量和 burst 状态；开启时才展示自适应热池指标。
- provider 模型测试的候选顺序为 `scheduled.chain(skipped)`，可调度项必须排在跳过项前。

关键文件：

- `apps/aether-gateway/src/ai_serving/planner/candidate_ranking.rs`
- `apps/aether-gateway/src/dispatch/pool_scheduler.rs`
- `apps/aether-gateway/src/handlers/admin/provider/pool/config.rs`
- `apps/aether-gateway/src/handlers/admin/provider/shared/support.rs`
- `frontend/src/features/pool/components/PoolAdvancedDialog.vue`
- `frontend/src/features/pool/utils/poolAdvancedDialog.ts`
- `frontend/src/views/admin/PoolManagement.vue`

历史警告：`569d8641a`、`116fbefae`、`975c97e7b` 等旧调度尝试已由 `0d9937ba2`、`65a2b4666` 等提交撤销。不要根据旧提交标题恢复 provider selection feedback 或已删除的调度分支；只保留当前净树中的不变量。

### P0：额度语义、倒计时与账号消耗统计

Fork 行为：

- pool key payload 从统一 `status_snapshot.quota` 读取额度窗口；窗口有容量时不能被陈旧的顶层 `exhausted`/metadata 标志错误拦截。
- OAuth plan tier 经 provider 类型归一化，pool 列表返回 plan/quota summary，并支持 `quota_available` 快速筛选。
- pool 页默认不强加 `imported_at` 排序，避免覆盖后端调度顺序。
- 独立“额度倒计时”页面汇总 Codex/Grok/兼容快照窗口，使用 `reset_at`、`reset_seconds`、`updated_at` 计算活动倒计时和进度。
- 独立“账号消耗统计”页面仅面向 Codex 号池；按浏览器时区查询今天、近 3/7/30 天和全部历史，展示请求数、输入/输出/cache/总 token、成本、平均值和最大/最小账号。
- 后端接口：`GET /api/admin/pool/{provider_id}/consumption-stats`；需要 provider catalog reader 和 usage reader。
- Postgres/SQLite/in-memory usage repository 必须能够按 provider API key 和独立时间窗口聚合上述字段。Codex quota window 重置不能删除历史 usage 事实。

关键文件：

- payload/路由：`apps/aether-gateway/src/handlers/admin/provider/pool_admin/{payloads,support}.rs`、`read_routes/consumption.rs`、`control/route/admin/observability_families.rs`。
- 数据：`crates/aether-data/**/usage*`、`summarize_provider_api_key_consumption_sql.sql`、`summarize_provider_api_key_window_usage_sql.sql`。
- 前端：`frontend/src/api/endpoints/pool.ts`、`features/pool/utils/quotaCountdown.ts`、`views/admin/{QuotaCountdown,PoolConsumptionStats,PoolManagement}.vue`、导航和路由。

合并规则：

- 上游 `f65ed2795` 已引入动态 Codex quota windows，不能保留只认识 `5h`/`weekly` 的硬编码模型。采用上游动态 window identity/grouping，再适配 fork 的倒计时、summary 和消费统计。
- 如果上游改变 `StoredProviderApiKeyWindowUsageSummary`，按新聚合模型重写消费接口，不要为了编译直接删除 fork 所需 token/cost 字段。
- `PoolManagement.vue` 必须保留上游动态 cycle groups 与样式，同时重放 fork 的刷新日志对话框、score toggle、额度可用筛选和页面入口。

### P0：OAuth Token 自动刷新控制与可观测性

全局配置契约：

| 配置键 | 默认值 | 运行时约束 |
|---|---:|---|
| `enable_oauth_token_refresh` | `true` | 总开关 |
| `oauth_token_refresh_lookahead_seconds` | `120` | 最大 30 天 |
| `oauth_token_refresh_interval_seconds` | `60` | 15 秒到 24 小时 |
| `oauth_token_refresh_concurrency` | `4` | 1–64 |
| `oauth_token_refresh_max_per_run` | `50` | 1–10000 |
| `oauth_token_refresh_proxy_node_id` | `null` | 跟随账号/系统、直连或指定代理节点 |

Provider 级覆盖位于 `provider.config.oauth_token_refresh`：`enabled`、`lookahead_seconds`、`interval_seconds`、`concurrency`、`max_per_run`、`proxy_node_id`。缺省继承全局值；provider 扫描间隔用 runtime KV stamp 限流。

运行时不变量：

- 只处理启用 provider、具备 refresh token、到达截止窗口且 invalid state 允许刷新的 OAuth key。
- 同时执行全局 semaphore 与 provider semaphore；全局/per-provider 每轮上限都必须生效。
- 自动刷新可覆盖代理节点；手动刷新和自动刷新共享持久化路径，但日志带 `refresh_context`。
- 到期时间既读取 key 字段，也回退到解密 auth config；凭据未变化记为 checked，变化才记为 refreshed。
- 每个账号记录 refreshed/checked/skipped/failed 事件；单轮账号事件最多 200 条；完成事件包含 scanned/eligible/resolved/refreshed/skipped/failed 汇总。
- 后台任务事件 API 支持 `order=desc`，MySQL/Postgres/SQLite/in-memory 实现必须保持同一排序语义。
- pool 管理页可编辑全局刷新参数与代理，合并展示 `maintenance.oauth.token.refresh` 和 `pool.quota.probe.worker` 最新账号日志（每任务取最新 run，降序最多 200 条，UI 合并后显示 60 条）。

关键文件：

- `apps/aether-gateway/src/maintenance/runtime/oauth_token_refresh.rs`
- `apps/aether-gateway/src/maintenance/runtime/workers.rs`
- `apps/aether-gateway/src/state/oauth.rs`
- `apps/aether-gateway/src/task_runtime/mod.rs`
- `apps/aether-gateway/src/handlers/admin/features/background_tasks/routes.rs`
- `crates/aether-data/**/background_tasks.rs`
- `apps/aether-gateway/src/handlers/admin/provider/summary/value.rs`
- `frontend/src/api/async-tasks.ts`
- `frontend/src/views/admin/PoolManagement.vue`

`build_admin_provider_summary_value` 改成 mutable `Map` 只是实现方式；真正契约是响应中保留 `oauth_token_refresh` 字段。

### P0：Provider Key“永不熔断”

- canonical capability：`disable_circuit_breaker: true`；兼容读取 `circuit_breaker_disabled` 和 `never_circuit_break`。
- 启用后，所有 `is_provider_key_circuit_open*`/`any_provider_key_circuit_open_at` 视为未打开；candidate selectability 同时忽略 zero-health skip。
- 该内部 capability 不出现在对用户公开的模型 capability 短名列表。
- Key 表单提供“永不熔断”开关；保存时只增删该键，必须保留 capabilities 中其他布尔项。

关键文件：`crates/aether-scheduler-core/src/{health,candidate/selectability,candidate/mod,lib}.rs`、`frontend/src/features/providers/components/KeyFormDialog.vue`、`apps/aether-gateway/src/handlers/public/system_modules_helpers/capabilities.rs`。

### P0：按权限查看自己的请求详情

安全契约：

- 新路由：`GET /api/users/me/usage/{usage_id}`，只能读取当前用户自己的记录；跨用户与不存在统一返回 404。
- `admin`、`audit_admin` 总能查看；普通用户必须由管理员设置 `feature_settings.usage_request_detail.enabled=true`。
- 用户自助更新 feature settings 时不能修改此受保护开关；管理员用户表单可修改。
- `include_bodies` 缺省为 `true`；body reference 按 capture state 解析。自助详情只暴露 client request/client response，不暴露 provider request/response、routing、settlement、trace。
- Authorization、cookie、各类 API key header 必须替换为 `[已隐藏]`。
- 只有 admin 看实际成本；普通用户详情禁用 cURL 导出和 replay。
- 前端 detail cache key 必须包含 `admin`/`self` scope，避免同一 request id 的权限域缓存串用。

关键文件：

- 后端：`control/route/public_support.rs`、`handlers/public/support/user_me_{routes,usage}.rs`、`handlers/shared/normalize.rs`。
- 前端：`frontend/src/utils/featureSettings.ts`、`stores/auth.ts`、`views/shared/Usage.vue`、`features/usage/components/{UsageRecordsTable,RequestDetailDrawer}.vue`、`features/users/components/UserFormDialog.vue`、`api/dashboard.ts`。

上游 `664c063a0` 同时增强 admin usage detail、body capture、reasoning metadata 和前端 drawer。合并时采用上游详情模型，再保留上述 self-scope、所有权校验、脱敏和 UI 限权；禁止直接保留 fork 旧版 drawer 覆盖上游增强。

### P1：本地镜像构建、Tunnel 打包与 fork 发布

`deploy.sh` 是“构建镜像”脚本，不再负责 compose restart：

- 根据源码、构建清单、`AETHER_BUILD_VERSION` 和 tunnel 参数计算 `.code-hash`，无变化复用现有镜像。
- `--force` 强制构建；`--tag/-t` 在保留 `latest` 的同时追加合法自定义 tag。
- `AETHER_TUNNEL_MODE=source|release|none`：从当前源码构建、按 release tag 下载 amd64/arm64 包、或不打包 tunnel。
- `DOCKER_BUILD_CACHE` 默认 `0`。`--build-cache` 才启用本地 BuildKit cache；不支持 external cache 的 builder 回退到镜像/local layer cache。禁用时清理由脚本创建的缓存。
- `--no-cache`/`DOCKER_NO_CACHE=1` 向 Docker 传 `--no-cache` 并强制构建。

镜像/编排契约：

- `Dockerfile.app.local` 使用国内镜像源、cargo/npm cache mount 和 scratch runtime；按 tunnel mode 打包依赖；保留 `/etc/passwd`/`group`，运行用户为 `0:0`。
- `docker-compose.build.yml` 提供 Postgres + Redis + app 的本地源码构建部署，并提供 profile 控制的 MySQL。
- `.docker-cache/` 同时在 `.dockerignore`/`.gitignore` 中排除。
- release workflow 不使用硬编码上游镜像名，发布到 `${REGISTRY}/${GITHUB_REPOSITORY,,}`；docker job 具有 `packages: write`，启用 GHA build cache。
- README 与公开 Guide 中的本地构建说明必须与脚本参数同步。

关键文件：`deploy.sh`、`Dockerfile.app.local`、`docker-compose.build.yml`、`.github/workflows/release.yml`、`.dockerignore`、`README.md`、`frontend/src/views/public/guide/**`。

### P1：前端部署后 chunk 恢复

- `isModuleLoadFailure` 识别常见 dynamic import/ChunkLoadError 文案。
- `importWithRetry` 最多重试 3 次；第二次前清浏览器 Cache Storage；最终用 `_t=<timestamp>` + `location.replace` 绕过旧入口缓存。
- `App.vue` 的全局错误处理使用同一识别与 reload 函数，避免仅匹配单一浏览器错误文本。

关键文件：`frontend/src/utils/importRetry.ts`、`frontend/src/App.vue`。

### P1：安全与运维修正

- 清理任务把 detail/compressed/header retention 下限设为 1 天，log retention 下限设为 30 天，避免配置 `0` 导致破坏性立即清理：`apps/aether-gateway/src/maintenance/runtime/config.rs`。
- `pool.quota.probe.worker` 注册为 scheduled interval task：`apps/aether-gateway/src/task_runtime/mod.rs`。
- 后台任务事件支持 asc/desc，见 OAuth 章节。
- pool consumption route 曾在上游合并中丢失，`3e9239415` 专门恢复。以后处理 router/mod 拆分时必须做路由烟测，不能仅确认 handler 文件仍存在。

### P2：低风险维护差异

这些不是必须保留的产品功能；若上游已有更干净的等价实现，优先接受上游：

- `ScatterChart.vue`、`PercentileChart.vue`、`useTTLAnalysis.ts` 的 Chart.js 类型收窄/断言。
- `useEscapeKey.ts` 使用 `HTMLElement.isContentEditable`。
- `api/client.ts` 的泛型从 `unknown` 放宽为 `any`、`buildCacheKey` 参数改为 `object`；只在上游版本无法通过现有调用时保留。
- provider summary 从 `json!` 改为 mutable `Map` 的纯重构部分。
- generated baseline SQL 尾部空行差异。
- `.gitignore` 中 `.cursor`、`.trellis`、`AGENTS.md`、`.agents` 是本地工具忽略规则。
- `.skills/upstream-merge/**` 本身只存在于 fork，合并时保留。

## 当前 24 个双边改动路径

以下路径在本快照中 fork 和 upstream 都从 merge-base 修改过，是最高概率冲突区：

```text
apps/aether-gateway/src/ai_serving/pure/mod.rs
apps/aether-gateway/src/handlers/admin/provider/pool_admin/payloads.rs
apps/aether-gateway/src/handlers/public/support/user_me_usage.rs
apps/aether-gateway/src/handlers/shared/catalog.rs
apps/aether-gateway/src/tests/control/admin/pool.rs
crates/aether-admin/src/system.rs
crates/aether-ai/formats/src/api.rs
crates/aether-billing/src/event_enrichment.rs
crates/aether-billing/src/pricing.rs
crates/aether-billing/src/service.rs
crates/aether-data/adapters/postgres/src/usage/mod.rs
crates/aether-data/adapters/sqlite/src/usage.rs
crates/aether-data/contracts/src/repository/usage/types.rs
crates/aether-data/runtime/src/repository/usage/memory.rs
crates/aether-usage/runtime/src/write.rs
frontend/src/api/dashboard.ts
frontend/src/features/usage/components/RequestDetailDrawer.vue
frontend/src/features/usage/components/UsageRecordsTable.vue
frontend/src/features/usage/components/__tests__/UsageRecordsTable.spec.ts
frontend/src/utils/featureSettings.ts
frontend/src/utils/providerKeyQuota.ts
frontend/src/views/admin/PoolManagement.vue
frontend/src/views/admin/__tests__/PoolManagement.codex-cycle-stats.spec.ts
frontend/src/views/shared/Usage.vue
```

按行为分组解决：

| 组 | 上游必须接入 | Fork 必须重放 | 默认决策 |
|---|---|---|---|
| AI exports | Codex cache identity headers | transcription exports、multipart、plan/report kinds | hybrid |
| Pool/quota | 动态 quota windows、cycle groups、样式 | cache affinity、score toggle、倒计时、消费统计、刷新对话框 | hybrid，以 upstream window 模型为底 |
| Billing/usage | processing-tier multiplier、reasoning metadata、capture 更新/清空语义 | audio duration、转写 request type、pool key 聚合、自助详情 | hybrid，以 upstream persistence/settlement 为底 |
| Usage UI | 新 pricing/detail/timeline 展示 | self scope、权限开关、禁 cURL/replay、转写会话展示 | hybrid，以 upstream 组件为底 |
| Feature settings | 上游新增 feature keys | `usage_request_detail` 且自助不可改 | hybrid，逐键 merge |

## 当前尚未合入的上游功能

截至本快照，`upstream/main` 比 fork 多 20 个提交。冲突处理时不能把这些当成 fork 回归而删除：

- `d9796d502`、`5b332da7d`、`75795c6fb`、`6c33b8d8f`：Codex/OpenAI Responses 缓存身份一致性。
- `f65ed2795`：动态 quota windows。
- `664c063a0`：usage audit metadata 与详情视图增强。
- `0be380243`、`373ebf26d`：processing tier multipliers 与零倍率修正。
- `7851503fb`、`7b56546e2`：同步 Responses 流 capture/finalize 修正。
- `6b707f29a`：S3 backup User-Agent 配置。
- `cd8de1aa1`：OpenAI Chat Completions 中 Developer role → `system`。
- `5dda34c66`、`ed27d404a`、`88a057b8d`：usage fast tier badge 样式。
- `e558f55cd`、`a6c6f14b0`：pool cycle stats 样式调整。

每次合并后更新本节和上面的 refs/counts/双边路径列表。

## 配置与 API 契约速查

```text
API format:  openai:transcription
AI route:    POST /v1/audio/transcriptions
Pool stats:  GET /api/admin/pool/{provider_id}/consumption-stats
Self detail: GET /api/users/me/usage/{usage_id}?include_bodies=true|false

Provider config:
  pool_advanced.score_ranking_enabled (default true)
  oauth_token_refresh.{enabled,lookahead_seconds,interval_seconds,concurrency,max_per_run,proxy_node_id}

Key capability:
  disable_circuit_breaker=true
  legacy read aliases: circuit_breaker_disabled, never_circuit_break

User feature:
  feature_settings.usage_request_detail.enabled

Task keys:
  maintenance.oauth.token.refresh
  pool.quota.probe.worker
```

## 合并后验证清单

先跑编译基线：

```sh
cd frontend && npm run build
cargo check --workspace
```

再按冲突面执行行为验证：

```sh
# 转写 multipart、同步/流式、模型映射与 failover
cargo test -p aether-ai-formats transcription
cargo test -p aether-gateway transcription

# 永不熔断与 pool 调度
cargo test -p aether-scheduler-core disable_circuit_breaker
cargo test -p aether-gateway pool

# 前端关键契约
cd frontend
npm run test:run -- \
  src/features/usage/conversation/__tests__/openai.spec.ts \
  src/features/usage/components/__tests__/UsageRecordsTable.spec.ts \
  src/features/pool/utils/__tests__/poolManagementState.spec.ts \
  src/views/admin/__tests__/PoolManagement.codex-cycle-stats.spec.ts
```

还必须做四个烟测：

1. 向 `/v1/audio/transcriptions` 上传含二进制和伪 boundary 的音频，确认上游收到的文件字节不变且 model 已映射；分别测 `stream=false/true`。
2. 打开 pool 管理页，确认 score toggle、OAuth 刷新配置、最新降序日志、动态 quota windows 同时存在。
3. 普通用户关闭/开启 `usage_request_detail` 各测一次：关闭为 403；开启只能看自己的记录，header 已脱敏且无 cURL/replay。
4. 访问 `/admin/quota-countdown`、`/admin/pool-consumption`，确认路由可达且 consumption 历史不随 Codex quota window 重置丢失。

如果上游改变了任一契约或测试命令，更新本文件，不要保留失效说明。

# Aether Fork Delta 手册

## 1. 基线快照

| 项 | 值 |
|---|---|
| fork 分支 | `rust` |
| fork code baseline（HEAD） | `1be2c4c3cd16c7050050a681b741d2d42a0fd854` |
| upstream HEAD | `e58570d79d4fe47c087c97502640f4f788b788a3` |
| merge-base | `e58570d79`（= upstream HEAD，已完全合入） |
| 分叉计数 | fork-only 198，upstream-only 0 |
| fork-only 路径 | 252 个，`+22388/-748` |
| upstream-only 路径 | 0 |
| 快照日期 | 2026-09-09 |

本轮 4 个提交：
- `90db3fe71` merge 第 1 批（`HEAD..b5ed80227`，63 个上游提交，23 个冲突）
- `e57ff5a7d` merge 第 2 批（`b5ed80227..e58570d79`，30 个上游提交，21 个冲突）
- `29f5971b7` feat(observability) 账号级任务事件独立存储（后端）
- `1be2c4c3c` feat(pool) 刷新工作台读取新端点（前端）

## 2. Fork 特有功能清单

### P0 功能

1. **OpenAI Audio Transcriptions 端到端**
   - 外部契约：`openai:transcription`、`POST /v1/audio/transcriptions`、multipart 二进制保真、同步+流式直通、usage 记 `audio` 并暴露计费维度 `audio_duration_seconds`。
   - 关键文件：`crates/aether-billing/src/{default_rule,pricing,event_enrichment,service}.rs`、`crates/aether-usage/runtime/src/usage_mapper.rs`、`crates/aether-ai/formats/**`、`apps/aether-gateway/src/ai_serving/**`。
   - 合并规则：上游重构 registry/planner 时采用新结构但重接格式与保真；`pure/mod.rs` 与 `api.rs` 导出必须并存；transcription SSE 原样直通；计费重构补回 `audio_duration_seconds`。

2. **额度倒计时 + 账号消耗统计**
   - 外部契约：`GET /api/admin/pool/{provider_id}/consumption-stats`、读取统一 `status_snapshot.quota` 窗口、Codex 重置不丢历史 usage、前端独立倒计时与消耗统计页面及 `quota_available` 筛选。
   - 关键文件：`apps/aether-gateway/src/handlers/admin/provider/pool_admin/read_routes/consumption.rs`、`crates/aether-data/**/usage*`、`frontend/src/views/admin/{QuotaCountdown,PoolConsumptionStats}.vue`。
   - 合并规则：适配上游动态 quota windows 与 cycle groups；聚合模型变动时按新模型重写接口，保留 token/cost 聚合字段与页面入口。

3. **自助请求详情**
   - 外部契约：`GET /api/users/me/usage/{usage_id}`、受管理员开关 `feature_settings.usage_request_detail.enabled` 控制、跨用户 404、仅暴露客户端请求响应并脱敏鉴权头、普通用户禁用成本/cURL/replay。
   - 关键文件：`apps/aether-gateway/src/handlers/public/support/user_me_{routes,usage}.rs`、`frontend/src/features/usage/components/{UsageRecordsTable,RequestDetailDrawer}.vue`。
   - 合并规则：`RequestDetailDrawer.vue` 同时保留 `detailScope` 与 `summaryRecord`，`Usage.vue` 传 self/admin scope；禁止用任一侧整文件覆盖混合契约。

4. **Provider Key 永不熔断**
   - 外部契约：内部 capability `disable_circuit_breaker: true`（兼容读取 `circuit_breaker_disabled`/`never_circuit_break`），熔断判定视为未开且 candidate selectability 忽略 zero-health skip。
   - 关键文件：`crates/aether-scheduler-core/src/{health,candidate/selectability,lib}.rs`、`frontend/src/features/providers/components/KeyFormDialog.vue`。
   - 合并规则：Key 表单保存时保留 capabilities 中其他布尔项；上游调度/健康重构时保留该 capability 旁路逻辑，不作为公开模型 capability。

5. **OAuth Token 自动刷新控制与可观测性**
   - 外部契约：全局配置（`enable_oauth_token_refresh` 等 6 键）与 Provider 覆盖、双层限流信号量、代理覆盖、每账号事件（refreshed/checked/skipped/failed）、后台任务事件 `order=desc`。
   - 关键文件：`apps/aether-gateway/src/maintenance/runtime/oauth_token_refresh.rs`、`state/oauth.rs`、`task_runtime/mod.rs`、`crates/aether-data/**/background_tasks.rs`。
   - 合并规则：保持全局/Provider 扫描间隔与限流契约；后台任务事件 API 保持 `order=desc` 排序；账号事件详见第 8 项独立存储。

6. **`ignore_pool_cooldown`**
   - 外部契约：`pool_advanced.ignore_pool_cooldown` 运行时开关，关闭全部 `set_pool_cooldown` 写入（`score_ranking_enabled` 与 `skip_exhausted_accounts` 仅为配置兼容，运行时由上游接管）。
   - 关键文件：`apps/aether-gateway/src/handlers/admin/provider/pool/runtime/writes.rs`、`crates/aether-pool-core/src/scheduler.rs`。
   - 合并规则：冲突时该开关必须包住所有 `set_pool_cooldown` 调用；上游冷却原因细分（如 429 quota vs rate limit）放入开关内部，不得删除开关。

7. **号池调度不变量**
   - 外部契约：cache-affinity 命中提升 pool group priority 到最高、routed policy 继承 `keep_priority_on_conversion`、`probing_enabled` 关闭时不显示虚假热池指标、模型测试候选顺序 `scheduled.chain(skipped)`。
   - 关键文件：`apps/aether-gateway/src/ai_serving/planner/candidate_ranking.rs`、`dispatch/pool_scheduler.rs`、`frontend/src/features/pool/**`。
   - 合并规则：上游 routing policy 调整时必须维持 priority 继承与 cache-affinity 提升双生效；测试候选顺序保持可调度优先于跳过项。

8. **账号级任务事件独立存储**（本轮新增）
   - 外部契约：`provider_key_task_events` 表、`GET /api/admin/tasks/{task_key}/account-events`，隔离账号级高频事件，避免污染 `background_tasks` 审计与触发白名单拦截。
   - 关键文件：`migrations/20260909000000_create_provider_key_task_events.sql`、`crates/aether-data/**/provider_key_task_events.rs`、`apps/aether-gateway/src/handlers/admin/features/background_tasks/account_events.rs`。
   - 合并规则：`background_tasks` 事件白名单保持严格安全校验，账号事件全部写入独立表；迁移必须包含 `IF NOT EXISTS`，不进 generated baseline。

### P1 功能

9. **本地镜像构建链**
   - 外部契约：`deploy.sh` 契约是纯构建镜像不做 compose restart（含 `.code-hash` 缓存、`--tag`、`AETHER_TUNNEL_MODE`、BuildKit 缓存控制），`Dockerfile.app.local`、`docker-compose.build.yml`、`publish-image.yml`。
   - 关键文件：`deploy.sh`、`Dockerfile.app.local`、`docker-compose.build.yml`、`.github/workflows/publish-image.yml`。
   - 合并规则：上游在已删除的 restart 尾块内的改动不构成恢复理由；保持纯构建契约，本地迁移使用 `DATABASE_MODE` 兼容。

10. **清理任务保留期下限与定时探测**
   - 外部契约：detail/compressed/header retention 下限 1 天，log retention 下限 30 天，避免配置 0 导致破坏性立即清理；`pool.quota.probe.worker` 注册为 scheduled interval task。
   - 关键文件：`apps/aether-gateway/src/maintenance/runtime/config.rs`、`apps/aether-gateway/src/task_runtime/mod.rs`。
   - 合并规则：上游清理配置重构时保留下限防呆；上游定时任务注册表变动时确保 quota probe worker 不漏注。

### P2 功能（上游有等价实现时优先让位）
Chart.js 类型收窄（`ScatterChart.vue` 等）、`useEscapeKey` (`isContentEditable`)、`api/client.ts` 泛型、`.gitignore` 本地工具规则（`.cursor`、`.agents` 等）、`.mcp.json`、`.codegraph/`、`.skills/upstream-merge/**`。

## 3. 已被上游吸收（不再是 fork 差异）

- `importWithRetry` 前端 chunk 恢复 — 上游已有 `frontend/src/utils/importRetry.ts` + `router/routes/helpers.ts`
- `cyber_continue_failover`
- Antigravity 自定义反代、fork TPS 修正 — 2026-08-26 已按要求回到 upstream baseline
- legacy backfill
- **MySQL / SQLite** — 本轮随上游 `2281f2b75` 移除，fork 现在只支持 PostgreSQL

## 4. 总体冲突策略

1. 默认禁止整文件 `ours`/`theirs`，优先语义混合 (manual hybrid)。
2. 安全加固 `579f2c7cc` 引入的边界/信封/校验结构属于 upstream baseline，后续冲突不得用旧版整文件覆盖。
3. **`background_tasks` 事件白名单不得放宽**：`sanitize_background_task_event_type` 和 `SAFE_BACKGROUND_TASK_METADATA_FIELDS` 只放行 fork 的 8 个事件类型和纯计数字段；标识符与自由文本走 `provider_key_task_events` 表。上游的 `run_sanitization_removes_sensitive_and_nested_metadata` 和 `event_sanitization_canonicalizes_type_message_and_payload` 两个测试**永远不要改**。
4. `cargo check --workspace` 不编译 `#[cfg(test)]`，上游改了 fork 测试构造的公共结构体时必须补跑 `CARGO_BUILD_JOBS=1 cargo check --workspace --all-targets`。
5. `deploy.sh` 的纯构建契约是 P1，上游在已删除的 restart 尾块内的改动不构成恢复理由。
6. fork 的迁移只放 `migrations/` 目录、不进 generated baseline，且必须 `IF NOT EXISTS`。

## 5. 运维警告

**凭证信封 v2 是单向升级**：上游 `handlers/shared/provider_catalog_credential.rs` 能读旧的裸 Fernet 并自动升级为 `aether-provider-catalog-credential-v2:` 信封，新镜像可以直接替换旧容器启动；但**跑过之后无法回滚到 m14.4**（旧版只认裸 Fernet，会报 `invalid Python Fernet outer base64 payload`）。上生产前必须 `pg_dump` 全库备份。

## 6. 配置与 API 契约速查

```text
API format:  openai:transcription
Routes:
  POST /v1/audio/transcriptions
  GET  /api/admin/pool/{provider_id}/consumption-stats
  GET  /api/users/me/usage/{usage_id}?include_bodies=true|false
  GET  /api/admin/tasks/{task_key}/account-events

Provider config:
  pool_advanced.score_ranking_enabled   (兼容读写；scheduler 不读取)
  pool_advanced.skip_exhausted_accounts (兼容读写；quota 耗尽一律阻断)
  pool_advanced.ignore_pool_cooldown    (fork 运行时有效；拦截全部 set_pool_cooldown 写入)
  oauth_token_refresh.{enabled,lookahead_seconds,interval_seconds,concurrency,max_per_run,proxy_node_id}

Key capability:
  disable_circuit_breaker=true (兼容读取: circuit_breaker_disabled, never_circuit_break)

User feature:
  feature_settings.usage_request_detail.enabled

Task keys:
  maintenance.oauth.token.refresh
  pool.quota.probe.worker
```

## 7. 合并后验证清单

```sh
# 1. 编译基线（只允许一个 rust 编译进程，串行执行）
cd aether-vscodex/web && npm install
cd frontend && npm run build
CARGO_BUILD_JOBS=1 cargo check --workspace
CARGO_BUILD_JOBS=1 cargo check --workspace --all-targets

# 2. 定向功能与冲突回归测试
CARGO_BUILD_JOBS=1 cargo test -p aether-data-contracts background_task
CARGO_BUILD_JOBS=1 cargo test -p aether-data provider_key_task_events
CARGO_BUILD_JOBS=1 cargo test -p aether-gateway --lib handlers::admin::provider::pool::runtime::writes
CARGO_BUILD_JOBS=1 cargo test -p aether-gateway --lib maintenance::runtime::pool_quota_probe
CARGO_BUILD_JOBS=1 cargo test -p aether-ai-formats transcription
CARGO_BUILD_JOBS=1 cargo test -p aether-scheduler-core disable_circuit_breaker
CARGO_BUILD_JOBS=1 RUST_MIN_STACK=8388608 cargo test -p aether-gateway users_me_usage

# 3. 前端定向测试
cd frontend && npm run test:run -- \
  src/features/pool/components/__tests__/PoolKeyDisplayPanels.spec.ts \
  src/views/admin/__tests__/PoolConsumptionStats.spec.ts \
  src/features/pool/components/__tests__/PoolSchedulingDialog.cache-affinity.spec.ts \
  src/features/usage/conversation/__tests__/openai.spec.ts
```

既存失败（非合并回归，无需在此修复）：
- `cargo check -p aether-provider-transport --all-targets` 报 7 个测试编译错误（来自 `49cc64748`、`2c8920200`）。
- `PoolManagement.codex-cycle-stats.spec.ts` 报 15 项失败（spec mock 缺少 `Gauge` 图标）。

## 8. 历史合并记录

| 日期 | Merge Commit | 上游 Commit | 一句话结论 |
|---|---|---|---|
| 2026-08-14 | `596e1830f` | `b7fca851b` | 解决 `pure/mod.rs` 冲突，接入动态 Codex catalog，保留 transcription 等全部 fork 功能 |
| 2026-08-21 | `eb159a090` | `16f96d73e` | 解决 14 处冲突，接入 Codex Live/Realtime，保留 transcription 二进制保真与 usage 聚合 |
| 2026-08-26 | `7530d3f2b` | `7892aa948` | 接入 Live/WS usage 与 quota 隔离，解决详情抽屉冲突，legacy backfill 由上游吸收 |
| 2026-08-26 | `ff29894df` | `7892aa948` | 回退轮：撤销 Antigravity 自定义反代与本地 TPS 修正，回到 upstream baseline |
| 2026-08-29 | `1fe868147` | `6ec077129` | 无文本冲突，接入 Gemini/Responses 工具与 Antigravity wire 修正，fork 功能逐项保留 |
| 2026-09-02 | `1a9453159` | `cae9aa413` | 解决 6 处冲突，接入 VSCodex 模块与 routing policy 统一，保留 transcription 与 self-scope |
| 2026-09-04 | `a169ba25d` | `27b0381a9` | 解决 4 处冲突，接入 quota 429 调度与数据库准备模式，保持 `deploy.sh` 纯构建与冷却忽略 |
| 2026-09-09 | `90db3fe71`<br>`e57ff5a7d` | `e58570d79` | 分两批合入 93 提交移除 MySQL/SQLite 并接入信封 v2；新增账号级任务事件独立存储并完全合入 |

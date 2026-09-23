# Aether Fork Delta 手册

## 1. 基线快照

| 项 | 值 |
|---|---|
| fork 分支 | `rust` |
| fork code baseline（本轮合并提交） | `a110bcba11712daa8e87284b91d8ba86c547c9d3` |
| upstream HEAD | `ec95989e0250cb55f338fe473b1eca2a0f81c130` |
| merge-base | `ec95989e0250cb55f338fe473b1eca2a0f81c130` |
| 分叉计数（含本轮合并提交，不含本轮文档提交） | fork-only 227，upstream-only 0 |
| fork-only 路径 | 290 个，`+26302/-981` |
| upstream-only 路径 | 0 个，`+0/-0` |
| 快照日期 | 2026-09-23（第八轮，合并后） |

### 第八轮合并前快照与合并后结论（2026-09-23）

| 项 | 值 |
|---|---|
| fork 分支 / 当前提交 | `rust` / `e728af61be216909e3b54428b60520569e080791` |
| upstream 目标 | `upstream/main` / `ec95989e0250cb55f338fe473b1eca2a0f81c130` |
| merge-base | `ba7c9f8b270cce63b0515299076b30129d7d64b4` |
| 分叉计数 | fork-only 226，upstream-only 12 |
| fork-only 路径 | 290 个，`+26302/-981` |
| upstream-only 路径 | 39 个，`+1585/-188` |
| 直接重叠路径 | 16 个（见下方） |
| 合并状态 | 已完成：`a110bcba11712daa8e87284b91d8ba86c547c9d3`；**0 处文本冲突**，无需用户冲突选择 |

合并前待合入 upstream 提交（12 个，按拓扑顺序）：

```text
906baae88 fix(gemini): preserve tool thought signatures
67d041448 Merge pull request #836 from dalamudx/fix/gemini-thought-signature-replay
0486435f1 feat(usage): 展示调度跳过候选及原因并补齐手机端提示
0b7c7f94a Merge pull request #833 from AAEE86/feat/usage-skipped-candidates
69930a605 fix(codex): preserve explicit service tiers and adapt usage badges
f86dd1046 Merge pull request #841 from zhefox/fix/codex-service-tier-passthrough
f960bbd2c feat: expose upstream response model in usage records
e3c01fb55 fix: avoid usage payload json recursion overflow
70d1a4ab7 fix: import usage body capture state in tests
07cb401fd fix: extract nested provider response models
7f5e1a64f merge: integrate usage response models with service tier badges
ec95989e0 Merge pull request #842 from zhefox/fix/usage-response-model-conflicts
```

直接重叠路径：

```text
apps/aether-gateway/src/execution_runtime/transport.rs
apps/aether-gateway/src/handlers/public/support/user_me_usage.rs
crates/aether-admin/src/observability/usage.rs
crates/aether-data/adapters/postgres/src/usage/tests.rs
crates/aether-data/contracts/src/repository/usage/metadata_policy.rs
crates/aether-data/contracts/src/repository/usage/types.rs
crates/aether-usage/runtime/src/request_metadata.rs
crates/aether-usage/runtime/src/write.rs
frontend/src/api/dashboard.ts
frontend/src/api/me.ts
frontend/src/api/usage.ts
frontend/src/features/usage/components/RequestDetailDrawer.vue
frontend/src/features/usage/components/UsageRecordsTable.vue
frontend/src/features/usage/components/__tests__/UsageRecordsTable.spec.ts
frontend/src/features/usage/composables/useUsageData.ts
frontend/src/views/shared/Usage.vue
```

合并后审计结论：

- `a110bcba1` 是基于 `e728af61b` 的 `--no-ff` 合并，已完整纳入 `upstream/main` 的 12 个提交；合并后 `upstream-only` 为 0。
- upstream 本轮能力：usage 记录新增 `response_model`（`request_metadata.provider_response_model`，仅在请求/响应 body 均为权威完整捕获且模型不同时写入）、调度跳过候选标记与原因（`has_skipped_candidate`、`skipped_candidate_reasons`、状态筛选 `has_skipped_candidate`）、Codex 显式 service tier 透传、Gemini 工具 thought signature 保留。
- upstream 本轮**没有删除任何 public 符号**，也没有给 fork 测试构造的公共结构体（`UsageTimeSeriesQuery`、`ProviderApiKeyWindowUsageRequest`、`StoredProviderApiKeyWindowUsageSummary`、`UsageProviderPerformanceQuery`）新增字段，规则 7 与规则 10 均未触发；本轮无合并回归修复。
- 16 个重叠路径逐个语义复核，双方改动均为不相交的增量：
  - 后端 8 个：fork 的 `user_agent` 持久化/首字节保留、`reasoning_tokens`、号池窗口统计字段、自助详情接口与 upstream 的 `response_model` 提取/清洗/透出互不干扰；`metadata_policy.rs` 与 `request_metadata.rs` 的白名单两侧新增键并存。
  - P0 第 3 项：fork 的 `build_users_me_usage_detail_payload` 以 `build_users_me_usage_record_payload` 为底，自动继承 upstream 新增的 `response_model`；`RequestDetailDrawer.vue` 的 `detailScope` 与 `summaryRecord` 保留，`Usage.vue` 仍传 `:detail-scope` 与 `:can-view-detail`。
  - upstream 在 `HorizontalRequestTimeline.vue` 新增的跳过原因只来自 admin trace API（`/api/admin/monitoring/trace/{id}`），普通用户自助详情不会拿到候选/调度信息。
  - `UsageRecordsTable.vue`：upstream 的跳过候选标记与响应模型 tooltip 和 fork 的 `canViewDetail` 行点击门控位于不同行；upstream 新增测试全部经 `mountUsageRecordsTable` helper 挂载，已携带 fork 的 `canViewDetail: true`。
- fork P0 第 1–8 项与 P1 第 9–10 项逐项存在性复核通过：transcription 格式与 `audio_duration_seconds` 计费、consumption-stats 处理器与两个前端页面、`usage_request_detail` 开关与 `detailScope`、`disable_circuit_breaker`、OAuth 刷新配置、`ignore_pool_cooldown`、`keep_priority_on_conversion`、账号级任务事件迁移与路由、`deploy.sh` 纯构建契约（无 compose/restart）、`pool.quota.probe.worker` 全部保留。
- **本轮没有 fork 功能性 delta 变化**；响应模型与跳过候选展示属于 upstream 能力，不加入 fork 特有功能清单。
- 合并后的待合入 upstream 提交：0。

### 第七轮合并前快照与合并后结论（2026-09-20）

| 项 | 值 |
|---|---|
| fork 分支 / 当前提交 | `rust` / `f4140dacc303d1015f5c9c61bd41a7e21d40b893` |
| upstream 目标 | `upstream/main` / `ba7c9f8b270cce63b0515299076b30129d7d64b4` |
| merge-base | `fb25dde4c9783eef7017383f49cf4362b5904e59` |
| 分叉计数 | fork-only 224，upstream-only 4 |
| fork-only 路径 | 290 个，`+26217/-982` |
| upstream-only 路径 | 33 个，`+1379/-514` |
| 直接重叠路径 | 11 个（见下方） |
| 合并状态 | 已完成：`b18ea85799c81ca9f74b358ae362c0cb0dcf528a`；**0 处文本冲突**，无需用户冲突选择 |

合并前待合入 upstream 提交（4 个，按拓扑顺序）：

```text
166de3335 fix(responses): keep raw reasoning on content only
37e3a3668 Merge pull request #835 from Kayphoon/fix/responses-reasoning-content-only
a95f0d248 feat(stats): add user group usage views
ba7c9f8b2 Merge pull request #834 from dalamudx/feat/user-group-stats
```

直接重叠路径：

```text
apps/aether-gateway/src/control/route/admin/observability_families.rs
apps/aether-gateway/src/handlers/admin/observability/stats/analytics_routes.rs
apps/aether-gateway/src/handlers/admin/observability/stats/cost_routes.rs
crates/aether-ai/formats/src/formats/shared/stream_core/format_matrix.rs
crates/aether-data/adapters/postgres/src/usage/mod.rs
crates/aether-data/adapters/postgres/src/usage/tests.rs
crates/aether-data/contracts/src/repository/usage/types.rs
crates/aether-data/runtime/src/repository/usage/memory.rs
crates/aether-data/runtime/src/repository/usage/memory/tests.rs
frontend/src/api/usage.ts
frontend/src/i18n/messages.ts
```

合并后审计结论：

- `b18ea8579` 是基于 `f4140dacc` 的 `--no-ff` 合并，已完整纳入 `upstream/main` 的 4 个提交；合并后 `upstream-only` 为 0。
- upstream 新增用户组使用统计：新路由 `GET /api/admin/stats/leaderboard/user-groups`、三个 usage 查询结构新增 `user_ids: Option<Vec<String>>` 批量用户范围、`UserStats.vue` 重写并 i18n 化 `LeaderboardTable.vue`；另含 `fix(responses): keep raw reasoning on content only`。
- 11 个重叠路径已逐个语义复核。upstream 本轮**没有删除任何 public 符号**（`git diff` 删除行中无 `pub fn/struct/enum/const/trait/type` 与前端 `export`），因此不存在规则 7 的“静默取上游侧”风险；`LeaderboardTable.vue` 新增的 `showMemberCount`/`selectable`/`select` 均为带默认值的可选项，既有调用方 `CostAnalysis.vue` 不受影响。
- **合并回归修复 3 处**（均为 fork 调用点适配 upstream 新增的必填 `user_ids` 字段，语义上 fork 场景不按用户组取范围，故一律 `None`）：
  - `pool_admin/read_routes/dashboard.rs` 两处 `UsageTimeSeriesQuery` 字面量补 `user_ids: None`。
  - `repository/usage/memory/tests.rs` 中 upstream 新增的 `usage_analytics_filters_by_multiple_user_ids` 补 fork 独有的 `provider_id`/`provider_api_key_ids`；fork 的 `usage_analytics_filters_by_canonical_provider_and_key_cohort` 补 `user_ids`。
  - 注意：`UsageTimeSeriesQuery` 的 `provider_id` / `provider_api_key_ids` 是 fork 独有字段（P0 第 2 项号池消耗看板），upstream 侧不存在，upstream 每次新增该结构的构造点都会在 `--all-targets` 阶段暴露。
- fork P0 第 1–8 项与 P1 第 9–10 项逐项存在性复核通过：transcription 格式与 `audio_duration_seconds` 计费、consumption-stats 处理器与两个前端页面、`usage_request_detail` 开关与 `detailScope`、`disable_circuit_breaker`、OAuth 刷新配置、`ignore_pool_cooldown`、`keep_priority_on_conversion`、账号级任务事件迁移与路由、`deploy.sh` 纯构建契约、`pool.quota.probe.worker` 全部保留。
- **本轮没有 fork 功能性 delta 变化**；用户组统计属于 upstream 能力，不加入 fork 特有功能清单。
- 合并后的待合入 upstream 提交：0。

### 第六轮合并前快照与合并后结论（2026-09-17）

| 项 | 值 |
|---|---|
| fork 分支 / 当前提交 | `rust` / `b0dafb37a4c9dc836884688d8b9e35421304f3bd` |
| upstream 目标 | `upstream/main` / `fb25dde4c9783eef7017383f49cf4362b5904e59` |
| merge-base | `531f53b4437c5dd69b7a498eaf09f1fe414e027b` |
| 分叉计数 | fork-only 222，upstream-only 40 |
| fork-only 路径 | 290 个，`+26091/-982` |
| upstream-only 路径 | 165 个，`+13296/-728` |
| 直接重叠路径 | 29 个（见下方） |
| 合并状态 | 已完成：`c504f059f7e7e818cacf08b52d7c4a04ada55522`；冲突策略 `1C, 2C` |

合并前待合入 upstream 提交（40 个，按拓扑顺序）：

```text
28cd77eb5 fix(deepseek): 完整保留思考内容并移除空值补齐
30e36cd09 fix(deepseek): 仅按官方地址识别思考兼容
b748b5bfd Merge pull request #813 from AAEE86/fix-deepseek-reasoning-replay
60b89cc84 fix(payment): restore recharge crediting and balance refresh
23e0af7b1 fix(frontend): show Anti Gravity v1internal endpoint path
ea24d6191 fix(antigravity): omit agent requestType from v1internal envelope
c5adcf031 fix(responses): map raw reasoning into content, keep summary for CLI
dfe88e34e feat(providers): add multi-select batch delete for provider models
e4f89de90 feat(providers): pin associated models to the top of the associate dialog
7daf355e6 fix(pool): keep schedulable keys when stale inactive scores exist
f753f14fd style(pool): satisfy rustfmt for stale score test
e83399db2 feat(providers): add xAI provider with device code OAuth
04c4a9776 feat(xai): add native image and video endpoints
cc5050155 Merge pull request #1 from hkxiaoyao/fix/pool-stale-inactive-score
01acff077 fix(routing): preserve fixed order for streaming chat
6e6407160 Merge pull request #824 from wanzhao-ysy/fix/fixed-order-target-select
e7864e561 Merge pull request #822 from stabey/upstream-pr/xai-media
88df2a2ed Merge pull request #815 from wanzhao-ysy/fix/antigravity-endpoint-default-path
6e431e2ff Merge pull request #820 from Kayphoon/cursor/responses-reasoning-content-68bd
5a6692ade Merge pull request #818 from wanzhao-ysy/fix/antigravity-omit-agent-request-type
e9899200f Merge pull request #821 from Kayphoon/feat/provider-model-batch-delete-and-pin
a5456cdc3 fix(routing): 按调度配置所选模型筛选提供商
e5ab73bf3 test(usage): preserve terminal build release notification
c0ded116a Merge pull request #826 from zhefox/main
03496c46c fix(ai-formats): carry OpenRouter reasoning fields through chat conversion
e66dd00b8 test(frontend): make cross-tab refresh retry timing deterministic
53562fd9d Merge pull request #828 from zhefox/main
03b198d5a Merge pull request #825 from AAEE86/fix-scheduling-model-providers
fe1723d87 fix(responses): bound upstream tool call IDs
72a4bf340 Merge pull request #829 from zhefox/fix/responses-call-id-length
5842c7232 fix(usage): preserve full bodies before queue truncation
364692da5 Merge pull request #830 from zhefox/fix/usage-full-body-retention
4ff412903 Merge pull request #823 from stabey/fix/openrouter-reasoning-fields
fdf55525f fix(ai-formats): keep client-declared search tools as Gemini function declarations
6c92db2ba fix(antigravity): send googleSearch instead of the Gemini 1.5 retrieval tool
bcb230800 feat(ai-formats): deliver Gemini grounding to every client as native citations
681ce56c4 Merge pull request #831 from stabey/fix/gemini-native-search
5a55116b6 fix(antigravity): harden tool schemas and Claude thought replay
4124749a7 fix(ai-serving): route provider-aware normalization through root seams
fb25dde4c Merge pull request #832 from dalamudx/fix/antigravity-schema-thought-replay
```

直接重叠路径：

```text
apps/aether-gateway/src/ai_serving/planner/candidate_resolution.rs
apps/aether-gateway/src/ai_serving/planner/passthrough/provider/family/request.rs
apps/aether-gateway/src/ai_serving/planner/standard/openai/chat/decision/request.rs
apps/aether-gateway/src/ai_serving/pure/mod.rs
apps/aether-gateway/src/api/ai/registry.rs
apps/aether-gateway/src/constants.rs
apps/aether-gateway/src/control/route/ai.rs
apps/aether-gateway/src/data/state/testing/video_tasks.rs
apps/aether-gateway/src/dispatch/pool_scheduler.rs
apps/aether-gateway/src/executor/orchestration.rs
apps/aether-gateway/src/frontdoor_loop_guard.rs
apps/aether-gateway/src/handlers/admin/provider/pool_admin/payloads.rs
apps/aether-gateway/src/handlers/admin/provider/query/models/model_test.rs
apps/aether-gateway/src/handlers/admin/provider/write/normalize.rs
apps/aether-gateway/src/handlers/shared/catalog.rs
apps/aether-gateway/src/router.rs
crates/aether-ai/formats/src/api.rs
crates/aether-ai/formats/src/formats/registry.rs
crates/aether-ai/formats/src/formats/shared/mod.rs
crates/aether-ai/formats/src/formats/shared/routing.rs
crates/aether-ai/formats/src/formats/shared/stream_core/format_matrix.rs
crates/aether-data/adapters/postgres/src/usage/tests.rs
crates/aether-provider/transport/src/conversion.rs
crates/aether-provider/transport/src/lib.rs
crates/aether-provider/transport/src/request_url/mod.rs
frontend/src/api/endpoints/types/provider.ts
frontend/src/i18n/messages.ts
frontend/src/utils/providerKeyQuota.ts
frontend/src/views/admin/PoolManagement.vue
```

合并后审计结论：

- `c504f059f` 是基于 `b0dafb37a` 的 `--no-ff` 合并，已完整纳入 `upstream/main` 的 40 个提交；合并后 `upstream-only` 为 0。
- 29 个重叠路径已逐个对比合并提交与两个父提交。`pool_scheduler.rs` 保留 stale inactive score 修复和 fork 的 `ignore_pool_cooldown` 回归测试；`pool_admin/payloads.rs` 保留 upstream 的 xAI 额度显示及 fork 的 OAuth 刷新状态 helper/测试。
- upstream 新增的 xAI provider/device OAuth、原生 image/video、Gemini grounding citations、视频 GET 路由、reasoning/tool schema 修复和完整 usage body 保留；私网 endpoint、transcription、额度/usage、自助详情、永不熔断、OAuth 刷新、账号级任务事件、冷却忽略、调度不变量和纯构建部署契约均通过复核。
- **本轮没有 fork 功能性 delta 变化**；新增 xAI/Gemini/video 能力属于 upstream 能力，不加入 fork 特有功能清单。
- 合并后的待合入 upstream 提交：0。

### 第五轮快照与合并后结论

本轮已合入唯一提交 `531f53b44 feat(routing): simplify model scheduling configuration`。它新增统一的 routing scheduling policy 模型、编辑器和测试，并重构 `RoutingProfiles.vue`；相对上一轮 upstream 基线共 11 个路径、`+1955/-673`。

本轮与 fork-only delta 的直接重叠路径为 0，实际也未出现文本冲突。合并采用 upstream 的新 routing scheduling 配置模型；合并后审计确认以下 fork P0/P1 契约仍保留：私网 endpoint 放行、PostgreSQL 幂等迁移、`ignore_pool_cooldown`、transcription、额度统计、账号级任务事件和结构化 provider 失败返回。

第五轮合并后没有 fork 功能性 delta 变化；新增 routing scheduling UI/策略属于 upstream 能力，已纳入“已被上游吸收”清单。

### 第四轮合并后快照

`157a1ea2b` 是基于 `8260a8721` 的 `--no-ff` 合并提交，已完整纳入 upstream `95e4d0149` 之前的 10 个提交；当前没有待合入的 upstream 提交。

本轮已合入的 upstream 提交：

- `33ea4ebf1`：错误日志敏感信息脱敏。
- `d28dd8903`：恢复 legacy Provider Endpoint health 默认值及相关 API/UI 测试。
- `ecc16673e` / `3a8dadcd6` / `6aeadcd1d` / `28f61ec45` / `e9b64c3e9`：并发限制、高 RPM 路径、stream/usage 预算、Redis 流和测试夹具加固。
- `8aedf87aa`：修复 nightly workflow 中截断的 Buildx action SHA。
- `72aea7898` / `95e4d0149`：detail-log 合并及主线同步。

合并后重点审查结论：

- `apps/aether-gateway/src/execution_runtime/transport.rs`、`private_upstream.rs`、provider builders 和 WebSocket 路径：私网 endpoint 仍是显式、逐 endpoint/key、origin 精确匹配放行，没有恢复全局私网绕过。
- `frontend/src/features/providers/components/EndpointFormDialog.vue`：私网开关、config 合并和 target 提示仍在，前后端契约未断。
- `crates/aether-data/adapters/postgres/migrations/**` 和 `provider_catalog.rs`：PostgreSQL-only、幂等迁移、m14.4 schema 兼容修复，以及 `ignore_pool_cooldown` 的列/读写/bind 仍在。
- `maintenance/runtime/pool_quota_probe.rs`、`dispatch/pool_scheduler.rs`、`scheduler-core`：采用 upstream 的并发/调度加固，同时保留 fork 的冷却忽略和候选顺序不变量。
- provider-query 结构化失败返回、local model test 结构化失败返回和敏感信息脱敏逻辑均保留。
- 两处文本冲突采用手工混合：`data/state/testkit.rs` 保留 provider task event 字段并接入 upstream routing group fixtures；`maintenance/mod.rs` 同时导出 OAuth refresh 与 pool quota probe 类型。
- 合并后测试构造器补齐 upstream 新增字段；provider catalog 的源码检查改为读取当前实现文件，避免引用已删除的 `postgres.rs`。

第二轮合并（5 个上游提交，**0 冲突**）：
- `f23086834` merge（`e58570d79..8260a8721`：strategy failover controls、routing failover/model testing 加固、payment order query 复用、workspace lint 修复、Linux-only release）
- `88e673f29` fix(ci) 修复 release.yml 里未定义的 `GHCR_IMAGE`/`DOCKERHUB_IMAGE`（既存债务，非本轮引入）

同日第一轮的 7 个提交：
- `90db3fe71` / `e57ff5a7d` 分两批合入 93 个上游提交（安全加固 + 删 MySQL/SQLite）
- `29f5971b7` / `1be2c4c3c` 账号级任务事件独立存储（后端 + 前端）
- `916660359` refactor(oauth) worker 拆分，`9131d9c33` feat(oauth) Codex 默认生效 + 存量迁移
- `035cec388` / `2249187c4` / `4b47cd357` / `abb930483` 号池页续期状态、页头 handler 修复、per-provider 开关

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
   - 关键文件：`crates/aether-data/adapters/postgres/migrations/20260909000000_add_provider_key_task_events.sql`、`crates/aether-data/**/provider_key_task_events.rs`、`apps/aether-gateway/src/handlers/admin/features/background_tasks/routes.rs`。
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
- routing scheduling policy/editor — 上游 `RoutingSchedulingPolicyEditor`、`RoutingModelSelector` 和统一 scheduling policy 规则已纳入主线
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
7. **零冲突不等于零风险，重叠路径必须逐个语义复核**。上游删除某个符号、而 fork 仍有调用方时，git 会静默取上游侧且不产生任何冲突标记。实例：2026-09-09 第一轮，上游删了 `PoolManagementHeader.vue` 里三个按钮的 `triggerAction` handler、fork 保留了按钮，自动合并后按钮点击直接抛 TypeError，全程无冲突提示。合并后必须对 `comm -12` 得出的重叠路径逐个 `git diff --cached` 复核，重点看「上游删了什么、fork 还在不在用」。前端同类问题靠 `npm run test:run` 全量跑才能兜住。
8. 前端 handler/事件映射表要用 `Record<UnionType, ...>` 显式标注，让漏项在 `vue-tsc` 阶段暴露，而不是运行时。
9. 第六轮冲突映射固定记录为 `1C, 2C`：`pool_scheduler.rs` 保留 stale-score 与 `ignore_pool_cooldown` 两组测试；`pool_admin/payloads.rs` 保留 xAI 额度与 OAuth 刷新状态两组契约。
10. **`UsageTimeSeriesQuery` 是 fork 扩展过的上游结构**：fork 为号池消耗看板加了 `provider_id` 与 `provider_api_key_ids` 两个字段，upstream 侧没有。上游每次给该结构加必填字段（如第七轮的 `user_ids`），fork 的 `dashboard.rs` 构造点会断编译、而上游新写的测试构造点会因缺 fork 字段断编译，两类都只有 `cargo check --workspace --all-targets` 能一次性暴露。第七轮有一处测试构造点错误正是在 `cargo check --workspace` 通过之后才由 `--all-targets` 抓出，印证了规则 4。
11. **自助详情 payload 自动继承用户列表 payload**：`build_users_me_usage_detail_payload` 以 `build_users_me_usage_record_payload` 为底再覆盖敏感字段。upstream 每次给用户 usage 列表 payload 加字段（如第八轮的 `response_model`），自助详情会无冲突地自动暴露该字段。合并时必须复核新字段对普通用户是否安全；若涉及 provider/成本/调度信息，要在 detail builder 中显式置 `Value::Null`。前端 `UsageRecordsTable` 的 `canViewDetail` 是 fork 必填 prop，upstream 新增的测试若绕过 `mountUsageRecordsTable` helper 直接挂载，需要补该 prop。

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

# 4. usage 重叠路径回归（usage 元数据/记录 payload 与自助详情前端契约重叠时选用）
CARGO_BUILD_JOBS=1 cargo test -p aether-data-contracts --lib repository::usage
CARGO_BUILD_JOBS=1 cargo test -p aether-usage-runtime --lib request_metadata
CARGO_BUILD_JOBS=1 cargo test -p aether-admin --lib observability::usage
cd frontend && npm run test:run -- \
  src/features/usage/components/__tests__/UsageRecordsTable.spec.ts \
  src/features/usage/components/__tests__/RequestDetailDrawer.pricing.spec.ts
```

第八轮实际验证结果（2026-09-23）：

- `cd frontend && npm run build`：通过，1 分 11 秒；包含 VSCodex sync/build 和主前端 Vite build。两个依赖目录已存在，因此未运行 `npm install`。
- `CARGO_BUILD_JOBS=1 cargo check --workspace`：通过，3 分 57 秒，无警告。
- 上面第 4 组三条 Rust 定向测试串行执行，合计 5 分 3 秒：`aether-data-contracts` 63/63、`aether-usage-runtime` 23/23、`aether-admin` 46/46 通过；fork 的 `user_agent`/`reasoning_tokens` 测试与 upstream 的 `response_model` 测试在同一合并文件中均通过。
- 前端 `UsageRecordsTable.spec.ts` 52/52、`RequestDetailDrawer.pricing.spec.ts` 13/13 通过。
- `git diff --cached --check` 与未解决冲突检查通过；16 个重叠路径已完成语义审计。

第八轮未运行项目（unverified）：

- `CARGO_BUILD_JOBS=1 cargo check --workspace --all-targets`：规则 4 的触发条件（upstream 改了 fork 测试构造的公共结构体）本轮不成立，未运行。upstream 新增的 gateway 测试（`user_me_usage.rs`、`execution_runtime/transport.rs`、`tests/ai_execute/stream_cli/direct.rs`）只做了 helper 签名人工核对，未编译。
- 第 2 组全部 7 条 fork 功能定向测试，以及第 3 组 4 个前端定向测试。

第七轮实际验证结果（2026-09-20）：

- `cd frontend && npm run build`：通过，1 分 2 秒；包含 VSCodex sync/build 和主前端 Vite build。两个依赖目录已存在，因此未运行 `npm install`。
- `CARGO_BUILD_JOBS=1 cargo check --workspace`：通过，3 分 45 秒，无警告。
- `CARGO_BUILD_JOBS=1 cargo check --workspace --all-targets`：首跑在 `repository/usage/memory/tests.rs` 报 `E0063 missing field user_ids`，补字段后复跑通过。
- `CARGO_BUILD_JOBS=1 cargo test -p aether-data --lib repository::usage::memory`：41/41 通过（含 upstream 新增的 `usage_analytics_filters_by_multiple_user_ids` 与 fork 的 `usage_analytics_filters_by_canonical_provider_and_key_cohort`）。
- `CARGO_BUILD_JOBS=1 cargo test -p aether-data-postgres --lib usage`：135 通过、14 ignored。
- `CARGO_BUILD_JOBS=1 cargo test -p aether-gateway --lib pool_admin::read_dashboard`：5/5 通过（fork 号池看板，本轮改过构造点）。
- `CARGO_BUILD_JOBS=1 cargo test -p aether-gateway --lib admin::stats`：30/30 通过（upstream 新增 user-group 统计路由）。
- `cd frontend && npm run test:run -- src/api/__tests__/admin-analytics-cache.spec.ts`：3/3 通过。
- `git diff --check`、暂存区检查和未解决冲突检查均通过；11 个重叠路径已完成语义审计。

第七轮未运行项目（unverified）：

- `CARGO_BUILD_JOBS=1 cargo test -p aether-data-contracts background_task`
- `CARGO_BUILD_JOBS=1 cargo test -p aether-data provider_key_task_events`
- `CARGO_BUILD_JOBS=1 cargo test -p aether-gateway --lib handlers::admin::provider::pool::runtime::writes`
- `CARGO_BUILD_JOBS=1 cargo test -p aether-gateway --lib maintenance::runtime::pool_quota_probe`
- `CARGO_BUILD_JOBS=1 cargo test -p aether-ai-formats transcription`
- `CARGO_BUILD_JOBS=1 cargo test -p aether-scheduler-core disable_circuit_breaker`
- `CARGO_BUILD_JOBS=1 RUST_MIN_STACK=8388608 cargo test -p aether-gateway users_me_usage`
- 前端 4 个定向测试：`PoolKeyDisplayPanels.spec.ts`、`PoolConsumptionStats.spec.ts`、`PoolSchedulingDialog.cache-affinity.spec.ts`、`openai.spec.ts`。

第七轮非阻塞警告：

- `cargo test -p aether-gateway --lib` 首次构建耗时 10 分 41 秒，单次超过 600 秒工具超时，已改为后台执行后取回结果；不影响结论。
- 注意：`pool_admin/read_routes/dashboard.rs` 的测试模块在 test 列表中的路径是 `handlers::admin::provider::pool_admin::read_dashboard::tests`，按目录名 `read_routes` 过滤会匹配到 0 个测试。

第六轮实际验证结果（2026-09-17）：

- `cd frontend && npm run build`：通过；包含 VSCodex sync/build 和主前端 Vite build。两个依赖目录已存在，因此未运行 `npm install`。
- `CARGO_BUILD_JOBS=1 cargo check --workspace`：通过，4 分 10 秒。
- `CARGO_BUILD_JOBS=1 cargo check --workspace --all-targets`：通过，11 分 24 秒。
- `CARGO_BUILD_JOBS=1 cargo test -p aether-gateway --lib dispatch::pool_scheduler`：54/54 通过，测试运行 50.96 秒。
- `CARGO_BUILD_JOBS=1 cargo test -p aether-gateway --lib handlers::admin::provider::pool_admin::payloads`：6/6 通过，测试运行 0.14 秒。
- 合并解析阶段 `git diff --check`、暂存区检查和未解决冲突检查均通过；合并后 29 个重叠路径已完成语义审计。

第六轮未运行项目（unverified）：

- `CARGO_BUILD_JOBS=1 cargo test -p aether-data-contracts background_task`
- `CARGO_BUILD_JOBS=1 cargo test -p aether-data provider_key_task_events`
- `CARGO_BUILD_JOBS=1 cargo test -p aether-gateway --lib handlers::admin::provider::pool::runtime::writes`
- `CARGO_BUILD_JOBS=1 cargo test -p aether-gateway --lib maintenance::runtime::pool_quota_probe`
- `CARGO_BUILD_JOBS=1 cargo test -p aether-ai-formats transcription`
- `CARGO_BUILD_JOBS=1 cargo test -p aether-scheduler-core disable_circuit_breaker`
- `CARGO_BUILD_JOBS=1 RUST_MIN_STACK=8388608 cargo test -p aether-gateway users_me_usage`
- 前端 4 个定向测试：`PoolKeyDisplayPanels.spec.ts`、`PoolConsumptionStats.spec.ts`、`PoolSchedulingDialog.cache-affinity.spec.ts`、`openai.spec.ts`。

第六轮非阻塞警告：

- frontend build 提示 `caniuse-lite` 已 12 个月未更新；不影响本轮构建结果。

本轮实际验证结果（2026-09-10）：

- `cd frontend && npm run build`：通过；包含 VSCodex sync/build 和主前端 Vite build。
- `CARGO_BUILD_JOBS=1 cargo check --workspace`：通过。
- `CARGO_BUILD_JOBS=1 cargo check --workspace --all-targets`：通过；仅有既存的 `aether-admin` 测试未使用 import warning。
- `CARGO_BUILD_JOBS=1 cargo test -p aether-provider-transport --lib`：497/497 通过。
- `CARGO_BUILD_JOBS=1 cargo test -p aether-data-postgres --lib provider_api_keys_insert_values_match_bind_order`：1/1 通过。
- `git diff --check` 和未解决冲突检查：通过，未发现 `UU` 文件。

第五轮实际验证结果（2026-09-10）：

- `cd frontend && npm run build`：通过；包含 VSCodex build 和主前端 Vite build。
- `CARGO_BUILD_JOBS=1 cargo check --workspace`：通过。
- routing 前端定向测试：3 个文件、43/43 通过（含新 scheduling editor/policy 和 RoutingProfiles failover）。
- `CARGO_BUILD_JOBS=1 cargo test -p aether-routing-core --lib`：28/28 通过。
- 合并后关键 fork 文件存在性与 `git diff --check`：通过；无未解决冲突。

既存失败（非合并回归，无需在此修复）：
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
| 2026-09-09 | `f23086834` | `8260a8721` | 5 提交、0 冲突；接入 routing failover controls 与 workspace lint 修复，4 个重叠路径逐项语义复核后无 fork 功能变化 |
| 2026-09-10 | `157a1ea2b` | `95e4d0149` | 10 提交、2 处文本冲突；接入并发/stream/Redis/health/logging 加固，保留私网 endpoint、幂等迁移、`ignore_pool_cooldown` 和结构化 provider 失败契约 |
| 2026-09-10 | `a2a8847ad` | `531f53b44` | 1 提交、0 文本冲突；接入统一 routing scheduling policy/editor，审计确认 fork P0/P1 功能无变化 |
| 2026-09-17 | `c504f059f` | `fb25dde4c` | 40 提交、2 处文本冲突；采用 `1C, 2C` 手工混合，接入 xAI/Gemini/video 能力，审计确认 fork P0/P1 功能无变化 |
| 2026-09-20 | `b18ea8579` | `ba7c9f8b2` | 4 提交、0 文本冲突；接入用户组使用统计与 Responses reasoning 修正，修复 3 处 `user_ids` 构造点回归，审计确认 fork P0/P1 功能无变化 |
| 2026-09-23 | `a110bcba1` | `ec95989e0` | 12 提交、0 文本冲突；接入 usage 响应模型、调度跳过候选展示、Codex service tier 透传与 Gemini thought signature，16 个重叠路径复核无回归，fork P0/P1 功能无变化 |

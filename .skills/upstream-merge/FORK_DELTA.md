# Fork 差异与上游合并冲突手册

本文件记录 `dibin666/Aether` 的 `rust` 分支相对 `fawney19/Aether` `main` 的持有行为。合并上游时先读本文件；它描述的是必须显式复核的功能契约，不代表冲突中可以整文件选择 `ours`。

## 本轮合并前快照（2026-08-26）

- 当前分支：`rust`，代码基线 `e1a2ad732b41d5cb87f3325fa1b658a76c577631`。
- 上游基线：`upstream/main`，提交 `7892aa94853461c1e634f7a5babbb1280128720f`。
- merge-base：`16f96d73ecc72c0b75d59b36e9c54fba7924db9f`。
- 分叉计数：fork-only 173 个提交，upstream-only 15 个提交。
- 路径计数：fork 侧 279 个路径，upstream 侧 68 个路径，双方重叠 28 个路径。
- 当前待合入上游功能：OpenAI Live/WebSocket usage 记录统一与审计查询补齐、当前 Codex Realtime live 路由、模型级 429 quota 隔离与 Spark 污染额度自愈、legacy backfill 升级保留、Responses namespace tools 跨 Chat 保真、跨格式同步 JSON 响应收尾，以及数据库迁移 cutoff 对齐。
- 本轮文本冲突：待 `git merge --no-commit --no-ff` 后按行为分组检查；不预先选择 `ours` 或 `theirs`。

## 强制更新纪律

每次上游合并都必须执行两次复核：合并前用于规划冲突，合并完成并通过验证后用于更新本文件。后一次不能省略，即使结论是“特有功能没有变化”。

合并后更新流程：

1. 先完成冲突解决、前后端验证并创建不可变的 merge commit。
2. 以该 merge commit 为 fork code baseline，重新比较 `upstream/main`，检查特有功能是否新增、改变、被上游吸收或意外丢失。
3. 更新快照日期、fork code baseline、upstream commit、merge-base、分叉计数、双边路径、当前待合入上游功能、功能清单、冲突规则和验证命令。
4. 在“最近一次合并后复核”中明确记录新增/改变/移除项；没有功能变化时也要写“无功能差异变化”，不能只改 commit hash。
5. 将本文件和 `SKILL.md` 的必要调整放进独立的文档 commit，不 amend merge commit。这样本文件可以稳定引用不会因文档修改而变化的 merge commit。
6. 最终交付必须同时报告 merge commit、文档更新 commit 和复核结论；两者完成前不得推送或宣布合并完成。

## 历史合并后复核（2026-08-14）

- 合并基线：merge commit `596e1830f`，上游 `b7fca851b8c8c357d17d664433f061efaa37b0c9`。
- 冲突策略：`1C`；21 个合并前重叠路径中，只有 `apps/aether-gateway/src/ai_serving/pure/mod.rs` 产生文本冲突，手工同时保留 Audio Transcriptions 的 multipart/转写导出和上游动态 Codex catalog 导出；其余路径由 Git 自动合并后进行 P0 语义复核，未使用整文件 `ours`/`theirs`。
- 功能结论：无 fork 特有功能被删除或改变。Audio Transcriptions、OAuth 自动刷新、额度/消费统计、self-scope usage detail、永不熔断、chunk 恢复及 Responses/Usage 诊断继续保留。
- 本次上游接入：versioned dynamic Codex model catalogs、routing model overrides 与 allowed-scope 解耦、allowlist 编辑保存修复、Codex quota 并发更新保护及相关状态/测试改动。
- 路径级复核：合并后 merge-base 为上游 `b7fca851b`；upstream-only 为 0 个提交、0 个路径；fork-only 为 155 个提交、272 个路径；两棵最终代码树相差 272 个 fork 特有路径，合并后双边重叠为 0 个路径。
- P0 复核：`pure/mod.rs` 同时导出转写解析、multipart 保真、动态 Codex catalog 投影和 `CODEX_CLIENT_VERSION`；provider catalog、OAuth/quota、routing state 和 fork 的 usage/billing、pool scheduler、self-scope 权限边界均未丢失或改变。
- 验证：`cd frontend && npm run build` 通过；`cargo check --workspace` 通过；合并树和文档更新的 `git diff --check` 通过。非阻塞警告：`caniuse-lite` 数据已 11 个月未更新；`npm install` 报告 11 个依赖漏洞（2 个 critical），未在本次合并中擅自升级依赖。

## 最近一次合并后复核（2026-08-21）

- 合并基线：merge commit `eb159a090`，上游 `16f96d73ecc72c0b75d59b36e9c54fba7924db9f`，merge-base 同为该上游提交；`git rev-list --left-right --count HEAD...upstream/main` 为 fork-only 172、upstream-only 0。
- 冲突策略：`1C, 2C, 3C, 4C`。14 个文本冲突均采用手工混合：AI 导出/传输同时保留 Audio Transcriptions 与 Codex Live/OpenAI Realtime；管理端同时保留三类格式测试；usage 保留详细窗口统计并采用 `usage_available()` token/cost 语义；Usage 前端同时保留刷新快照保护和 WebSocket 筛选测试。
- 功能结论：fork 特有功能无删除或改变。Audio Transcriptions、OAuth 自动刷新、额度/消费统计、self-scope usage detail、永不熔断、chunk 恢复、Responses history 与 Usage 诊断继续保留；上游新增的 Codex Live、OpenAI Realtime、Responses continuation/DeepSeek reasoning 修复、Codex continuation binding 修复和认证加固已接入。
- 合并回归修复：将 fork 的 multipart `body_base64` 沿 pinned stream planner 传递，并让 Codex Live 的 JSON planner 调用显式传 `None`。
- 路径级复核：相对 `upstream/main` 的 fork-only 代码路径为 279 个；upstream-only 提交和路径均为 0；两棵最终代码树相差 279 个 fork 特有路径；合并后双方重叠路径为 0。
- P0 复核：格式导出同时保留 transcription 与 Realtime/Codex Live；provider URL/鉴权同时覆盖三类协议；Responses continuation、动态 quota、OAuth 限流/代理、usage/billing 跨数据库契约、cache affinity、Pool scheduler 与 self-scope 权限边界均未丢失或改变。
- 验证：`cargo fmt --all -- --check`、`cd frontend && npm run build`、`cargo check --workspace` 均通过；前端 `useUsageData.spec.ts` 18 项、管理端 API 格式测试 3 项、usage `usage_available` 定向测试 1 项均通过；合并树和文档更新的 `git diff --check` 通过。
- 非阻塞警告：Browserslist 的 `caniuse-lite` 数据已 11 个月未更新；`npm install` 报告 11 个依赖漏洞（1 moderate、8 high、2 critical）；管理端测试有既存的 pool quick-selector 未使用导入警告，均未在本次合并中擅自修复或升级依赖。

## 最近一次合并后复核（2026-08-26）

- 合并基线：merge commit `7530d3f2b`；合并回归修复 `73e49c8e2`；上游 `7892aa94853461c1e634f7a5babbb1280128720f`，merge-base 同为该上游提交。
- 冲突策略：`1C, 2C`。`1C`（详情权限与抽屉数据）手工混合保留 self-scope 权限边界、`detailScope` 和 `summaryRecord`；`2C`（刷新、筛选与分页）手工混合接入服务端 search/API-format/status 分页，同时保留 fork 的刷新快照保护和用户本地 retry/fallback 筛选。
- 功能结论：无 fork 产品功能被删除或改变。Audio Transcriptions、OAuth 自动刷新、额度/消费统计、self-scope 请求详情、永不熔断、chunk 恢复、Responses history/Usage 诊断、cache affinity、pool scheduler 和 Pool header 行为均保留；legacy backfill 文件及其测试已被上游等价吸收，不再属于 fork-only 路径。
- 本次上游接入：OpenAI Live/WebSocket usage 记录统一与审计查询、当前 Codex Realtime live 路由、模型级 rate-limit quota 隔离、Spark 污染额度自愈、已应用 legacy backfill 升级保留、Responses namespace tools 跨 Chat 保真、跨格式同步 JSON 响应收尾，以及数据库 snapshot migration cutoff 对齐。
- 合并回归修复：将 Codex Realtime live 测试中的 `resolve_ai_passthrough_sync_request_body` 更正为 serving crate 实际导出的 `resolve_ai_passthrough_request_body`。
- 路径级复核：以代码基线 `73e49c8e2` 计算，merge-base 为上游 `7892aa948`；`git rev-list --left-right --count 73e49c8e2...upstream/main` 为 fork-only 176、upstream-only 0；相对上游的 fork-only 路径为 278；两棵代码树相差 278 个 fork 特有路径；合并后双方重叠路径为 0。本次独立文档提交另增加 1 个非代码提交，因此工作分支 HEAD 的提交计数为 fork-only 177。相较合并前，legacy backfill SQL 与 backfill 测试 2 个路径已由上游吸收，回归修复使 live HTTP 测试路径新增为 1 个 fork-only 路径。
- P0 复核：转写 multipart/二进制保真与 same-format stream、动态 quota/消费统计、OAuth 限流/代理、usage/billing 跨数据库契约、self-scope 请求详情、`disable_circuit_breaker`、Responses continuation history、端到端/候选时序、cache-affinity pool group 提升及全局 `keep_priority_on_conversion` 继承均保留；Realtime 路由和 usage 统一已接入。
- 验证：`cargo fmt --all -- --check` 通过；`cd frontend && npm run build` 通过；`cargo check --workspace` 通过；前端 6 个定向测试文件共 82 项通过；`cargo test -p aether-ai-formats transcription` 的 10 项转写测试通过；`cargo test -p aether-scheduler-core disable_circuit_breaker` 通过。`users_me_usage` 首次测试编译发现并修复上述单符号命名回归；按内存约束（单个编译进程峰值约 8 GiB）停止了修复后的定向测试编译，未记录该测试的最终执行结果。
- 非阻塞警告：Browserslist 的 `caniuse-lite` 数据已 11 个月未更新；本轮未执行 `npm install`，未引入依赖升级或审计变更。

## 基线快照

快照日期：2026-08-26（已执行 `git fetch upstream`，完成合并、回归修复、验证和合并后复核）。

| 项目 | 值 |
|---|---|
| fork | `origin` → `git@github.com:dibin666/Aether.git` |
| upstream | `upstream` → `https://github.com/fawney19/Aether.git` |
| fork 分支 | `rust` |
| fork code baseline（已创建的 merge commit） | `7530d3f2b53c6743b8f4c09dc4abd23c26d4434f` |
| 合并回归修复 | `73e49c8e2` |
| upstream HEAD | `7892aa94853461c1e634f7a5babbb1280128720f` |
| merge-base | `7892aa94853461c1e634f7a5babbb1280128720f` |
| 分叉计数（以代码基线计） | fork-only 176，upstream-only 0；含本次文档提交的工作分支 HEAD 为 177 |
| fork 侧净改动 | 278 个路径（合并后相对 upstream/main） |
| upstream 侧净改动 | 0 个路径，`+0/-0` |
| 双边同时改动 | 0 个路径（合并前 28 个重叠路径已复核，2 个冲突组按 `1C, 2C` 手工混合） |

## 当前待合入上游功能

- 无。`upstream/main` 已完整合入 merge commit `7530d3f2b`；`git rev-list HEAD..upstream/main` 为 0。

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
6. 请求详情 UI 与上游再次冲突时，必须同时保留 `detailScope` 权限域和 `summaryRecord` 最新摘要协调；`Usage.vue` 同时传入两者，不能退回 admin-only 或取消 self-scope。
7. Agent Identity OAuth 与自动刷新再次冲突时，保留 fork 的全局/Provider 限流、代理覆盖和任务事件；接入上游的 Agent Identity task recovery、认证配置代际护栏及 CAS 持久化。普通 OAuth 的 `[REFRESH_FAILED]` 仍不可重试。

8. `68f2636fe` 已显式采用 upstream 的 pool scheduler 与 Pool header 冲突侧；未来不得把 score gate 或已移除的页头快捷入口当成当前不变量静默恢复，恢复前需要新的产品决策。
9. Responses continuation history 与端到端时序现已属于 upstream baseline。AI export 冲突必须同时保留 history hydrate/record/storage 与 transcription；Usage 冲突必须同时保留端到端/候选时序、reasoning token 和格式感知 TPS，并让“计速耗时”对应实际 TPS 分母。
10. `704a16fcf` 接入的 Responses routing、SSE event-only normalization 与 reasoning effort 校验边界现已属于 upstream baseline。后续冲突应在格式转换中保留显式 effort，只在最终同格式 provider 的实际映射模型上校验；transcription 的 multipart/二进制路径不得误入 JSON effort 校验。routed policy 必须继续继承全局 `keep_priority_on_conversion`，同时保留 fork 的 cache-affinity pool group 优先级提升。

## Fork 特有功能清单

本轮合并后复核（2026-08-26）确认：以下 fork 特有功能清单无增删，均继续由 `7530d3f2b` 与 `73e49c8e2` 提供；legacy backfill 已由上游等价吸收，上游新增能力已在合并后复核中单独记录。

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
- `crates/aether-ai/formats/src/api.rs` 与 `apps/aether-gateway/src/ai_serving/pure/mod.rs` 同时承载上游 Codex 缓存身份、Responses continuation history 和 fork transcription 导出；合并导出列表时三者必须并存。
- 上游标准流现在会将独立 `event:` 行的事件类型补入缺少 `type` 的 JSON data payload；该 normalization 仅用于标准格式转换，same-format transcription SSE 必须继续原样直通。
- 上游正在重构 processing-tier 计费和 usage body capture。以其新结算语义为主，再补回 `audio_duration_seconds`；不要用 fork 旧版 `pricing.rs`/`service.rs` 覆盖上游文件。

### P0：号池调度不变量

当前必须保留：

- cache-affinity 命中 pool group 时，把 rankable 的 provider/key/global-format priority 提升到最高优先级，避免软策略打散粘性。
- routed policy 的 ordering config 必须继承全局 `keep_priority_on_conversion`；该上游 override 与上述 cache-affinity pool group 提升同时生效，不能互相覆盖。
- routed pool policy 的 allowed-key overlay 优先于普通候选扫描；无定向 key 时按上游分页 score phase，再进入分配模式/策略扫描。
- `probing_enabled` 关闭时不显示虚假的热池目标、热池数量和 burst 状态；开启时才展示自适应热池指标。
- provider 模型测试的候选顺序为 `scheduled.chain(skipped)`，可调度项必须排在跳过项前。

已改变的兼容行为：

- merge `68f2636fe` 的冲突选择 `2B` 采用上游 scheduler。`pool_advanced.score_ranking_enabled` 及旧别名仍被配置解析器和高级设置 UI 接受/保存，但 `PoolKeyCursor` 不再读取该值；关闭开关不会跳过 score phase。后续文档和测试不得继续把该键描述为运行时门禁。
- `free_first`、`team_first` 等策略仍存在于后端配置和调度 UI；本次只采用了上游测试 fixture 的 applicable 模型，没有删除生产策略。

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
- `PoolManagement.vue` 必须保留上游动态 cycle groups 与样式，同时保留 fork 的倒计时、额度可用筛选、消费统计和页面入口。merge `68f2636fe` 的 `5B` 选择移除了页头刷新日志/编辑/账号批量操作快捷入口；score toggle 仅为兼容保存项，不再代表运行时门禁。

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
- 后台与 API 仍支持全局刷新参数、代理和 `maintenance.oauth.token.refresh` / `pool.quota.probe.worker` 最新账号日志；每任务取最新 run，降序最多 200 条，UI 数据模型合并后显示 60 条。
- merge `68f2636fe` 的 `5B` 选择使 `PoolManagementHeader` 不再发出 `refreshWorker`，因此 pool 页头没有打开刷新配置/日志对话框的快捷入口；底层对话框和 handler 仍存在。恢复入口属于新的产品决策，不能在后续合并中静默重放。

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

上游 `664c063a0` 的 admin usage detail、body capture、reasoning metadata 和 drawer 增强已在 `3cb7afba9` 接入；本次采用 `1C`，`RequestDetailDrawer.vue` 同时保留 `detailScope` 与 `summaryRecord`，`Usage.vue` 同时传入 self/admin scope 和摘要记录。以后禁止用任一侧整文件覆盖这一混合契约。

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

## 本轮双边改动路径（合并前）

本轮 `merge-base` 为 `16f96d73ecc72c0b75d59b36e9c54fba7924db9f`；fork-only 为 173 个提交、279 个路径，upstream-only 为 15 个提交、68 个路径，双方重叠 28 个路径。冲突解决前只记录事实，不预设选择：

```text
apps/aether-gateway/src/{api/ai/registry.rs,constants.rs,control/route/ai.rs,
  frontdoor_loop_guard.rs,handlers/proxy/websocket/live/planner.rs,
  handlers/public/support/user_me_usage.rs,orchestration/mod.rs}
crates/aether-admin/src/{observability/usage.rs,system.rs}
crates/aether-ai/formats/src/formats/{openai/mod.rs,registry.rs,shared/routing.rs,
  shared/stream_core/format_matrix.rs}
crates/aether-data/adapters/postgres/src/usage/tests.rs
crates/aether-data/runtime/{backfills/postgres/20260722140744_sync_legacy_enabled_from_is_active.sql,
  src/lifecycle/backfill/tests.rs}
crates/aether-scheduler-core/src/candidate/mod.rs
frontend/src/{api/dashboard.ts,api/endpoints/types/__tests__/api-format.spec.ts,
  api/endpoints/types/api-format.ts,api/me.ts,
  features/usage/components/RequestDetailDrawer.vue,
  features/usage/components/UsageRecordsTable.vue,
  features/usage/components/__tests__/UsageRecordsTable.spec.ts,
  features/usage/composables/__tests__/useUsageData.spec.ts,
  features/usage/composables/useUsageData.ts,i18n/messages.ts,views/shared/Usage.vue}
```

冲突分组与行为影响在 `git merge --no-commit --no-ff` 后依据 `git diff --cc`、stage 2/3 内容和 P0 契约补充；用户选择会以 `ours`、`theirs` 或 `manual hybrid` 记录。

本轮实际冲突分组与选择：

| 组 | 选择 | 合并后行为 |
|---|---|---|
| Usage detail scope / summary | `1C` manual hybrid | 保留普通用户 self-scope 详情、权限判断和 `detailScope`；同时保留上游 `summaryRecord` 数据连接。 |
| Usage server filters / refresh | `2C` manual hybrid | 普通用户的 search/API-format/status 使用上游服务端分页；retry/fallback 继续本地筛选；刷新路径保留 fork 的 `preserveOnFailure`/`preserveOnEmpty` 快照保护。 |

P0 复核结果：无 fork 产品功能差异变化；legacy backfill 及测试路径被上游等价吸收。转写、cache affinity、动态 quota/消费统计、OAuth 自动刷新、usage/billing 跨数据库契约、self-scope 请求详情、`disable_circuit_breaker`、chunk 恢复、Responses continuation history 与 Usage 端到端/候选时序均保留；当前 pool scheduler score phase 与 Pool header 行为未改变。新增上游 Live/WebSocket usage、Codex Realtime live 路由、quota 隔离/自愈、namespace tools 和跨格式同步 JSON 响应已进入最终树。

## 当前尚未合入的上游功能

- 无。`upstream/main` 已并入 merge commit `7530d3f2b`；`git rev-list HEAD..upstream/main` 为 0。

## 配置与 API 契约速查

```text
API format:  openai:transcription
AI route:    POST /v1/audio/transcriptions
Pool stats:  GET /api/admin/pool/{provider_id}/consumption-stats
Self detail: GET /api/users/me/usage/{usage_id}?include_bodies=true|false

Provider config:
  pool_advanced.score_ranking_enabled (兼容读写；merge 68f2636fe 后 scheduler 不读取)
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

再按冲突面串行执行行为验证，避免多个 Cargo 进程争用 package/artifact lock：

```sh
# Responses routing、reasoning effort 校验边界与 event-only SSE
cargo test -p aether-gateway routing_policy_inherits_global_conversion_priority_override
cargo test -p aether-ai-formats runtime_reasoning_effort_is_preserved_across_concrete_model_mapping
cargo test -p aether-ai-formats event_only_stream_types_convert_across_standard_formats

# 转写 multipart、Responses continuation history、同步/流式与 failover
cargo test -p aether-ai-formats transcription
cargo test -p aether-ai-formats response_history
cargo test -p aether-gateway transcription

# Usage hybrid：reasoning、端到端时序和 self-scope 路由
cargo test -p aether-admin usage_record_exposes_reasoning_tokens_from_dimensions
cargo test -p aether-admin admin_usage_payloads_project_end_to_end_timings_from_metadata
RUST_MIN_STACK=8388608 cargo test -p aether-gateway users_me_usage

# 永不熔断、pool 调度与 OAuth 自动刷新
cargo test -p aether-scheduler-core disable_circuit_breaker
RUST_MIN_STACK=8388608 cargo test -p aether-gateway pool
RUST_MIN_STACK=8388608 cargo test -p aether-gateway oauth_token_refresh

# settlement-aware SQLite 账号窗口聚合
cargo test -p aether-data-sqlite sqlite_usage_stats_rebuild_uses_canonical_terminal_totals
```

前端关键契约：

```sh
cd frontend
npm run test:run -- \
  src/api/endpoints/types/__tests__/api-format.spec.ts \
  src/features/providers/components/__tests__/KeyAllowedModelsEditDialog.loading.spec.ts \
  src/features/pool/components/__tests__/PoolManagementHeader.spec.ts \
  src/features/pool/utils/__tests__/poolManagementState.spec.ts \
  src/features/pool/utils/__tests__/poolSchedulingDialog.spec.ts \
  src/features/usage/conversation/__tests__/openai.spec.ts \
  src/features/usage/components/__tests__/UsageRecordsTable.spec.ts \
  src/views/admin/__tests__/PoolManagement.codex-cycle-stats.spec.ts
```

历史 `596e1830f` 验证结果（2026-08-14）：

- `cd frontend && npm run build`：通过，耗时约 20 秒；非阻塞警告为 `caniuse-lite` 数据已 11 个月未更新。
- `cargo check --workspace`：通过，耗时 3 分 26 秒。
- merge tree、合并提交与文档的 `git diff --check`：通过。
- 本轮仅 `pure/mod.rs` 发生实际文本冲突，已按 `1C` 完成手工混合；没有 merge-regression 修复，按技能的最小验证规则未扩展执行专项行为测试，文档列出的命令保留为后续回归清单。
- `npm install` 审计报告 11 个依赖漏洞（2 个 critical）；未执行自动修复，避免把依赖升级混入上游合并。
- 浏览器烟测未自动执行；当前会话未授权浏览器自动化，需人工完成下列检查。

本次 `7530d3f2b` / `73e49c8e2` 验证结果：

- `cargo fmt --all -- --check`：通过。
- `cd frontend && npm run build`：通过；警告为 `caniuse-lite` 数据已 11 个月未更新。
- `cargo check --workspace`：通过。
- 前端定向测试：6 个测试文件、82 项通过。
- `cargo test -p aether-ai-formats transcription`：10 项通过。
- `cargo test -p aether-scheduler-core disable_circuit_breaker`：1 项通过。
- `users_me_usage` 定向测试首次编译发现并修复了 `resolve_ai_passthrough_sync_request_body` 命名回归；修复后按内存约束停止了单进程测试编译，未记录最终执行结果。
- 合并树、回归修复和文档的 `git diff --check`：通过。
- 未执行 `npm install`、依赖升级或浏览器人工烟测。

还必须做六个烟测：

1. 向 `/v1/audio/transcriptions` 上传含二进制和伪 boundary 的音频，确认上游收到的文件字节不变且 model 已映射；分别测 `stream=false/true`。
2. 打开 pool 管理页，确认动态 quota windows、Provider 详情预取和“导入账号”存在；页头不再显示刷新日志、编辑 Provider/Endpoint、账号批量操作快捷入口。`score_ranking_enabled` 仍可保存，但不得据此预期跳过 score phase。
3. 普通用户关闭/开启 `usage_request_detail` 各测一次：关闭为 403；开启只能看自己的记录，header 已脱敏且无 cURL/replay。
4. 访问 `/admin/quota-countdown`、`/admin/pool-consumption`，确认路由可达且 consumption 历史不随 Codex quota window 重置丢失。
5. 使用同一 API Key 连续调用 OpenAI Responses，确认 `previous_response_id` 可从持久化 history 恢复；制造一次候选失败后确认 usage 详情保留端到端与成功候选时序，前端 tooltip 同时显示生成耗时和实际计速耗时。
6. 为 routed policy 启用全局 `keep_priority_on_conversion`，确认候选排序继承该设置；使用只有独立 `event:` 行携带类型的 Responses SSE，确认跨格式流正确转换；将请求映射到不支持目标 effort 的实际模型，确认 provider transport 在发送前拒绝候选。

如果上游改变了任一契约或测试命令，更新本文件，不要保留失效说明。

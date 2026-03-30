# Usage 已删除账号显示设计

## 目标

在 Aether 的使用记录页面中，仅针对“自动删除的 access_token 账号”，当对应的 `ProviderAPIKey` 已经被删除时，仍在当前密钥显示位置展示该账号的邮箱，并在后面追加一个“已删除” badge。

## 范围

仅处理这类记录：

- Usage 记录携带 `provider_api_key_id`
- 对应的 Provider API Key 已不存在
- 且能命中 `AccessTokenDeleteLog.deleted_key_id`
- 且删除日志中存在 `oauth_email`

不处理：

- 普通手动删除的 Provider Key
- 非 access_token 自动删除链路
- 没有删除日志或删除日志缺失邮箱的记录

## 现状问题

当前使用记录页管理员视图中，“提供商”列第二行显示的是 `provider_api_key_name`。该值依赖当前仍存在的 `ProviderAPIKey.name`。当自动删除链路删除了该 key 后，当前查询无法解析出名称，因此界面无法显示实际使用的账号是谁。

## 方案选择

采用“后端 Usage records 查询时做删除账号展示回填”的方案。

原因：

- 复用当前 analytics records 响应结构
- 前端只负责展示，不需要额外发删除历史请求
- 只在管理员视图补充所需信息，影响面最小

## 数据来源

后端从 `AccessTokenDeleteLog` 中读取：

- `deleted_key_id`
- `oauth_email`

匹配规则：

- `Usage.provider_api_key_id == AccessTokenDeleteLog.deleted_key_id`

## 后端改造

### 位置

- `src/services/analytics/query_service.py`

### 行为

在管理员 records 查询中：

1. 先按当前逻辑解析仍存在的 `ProviderAPIKey.name`
2. 再针对当前页的 `provider_api_key_id` 批量查询 `AccessTokenDeleteLog`
3. 若 key 当前已不存在，但命中删除日志且存在 `oauth_email`：
   - 将 `oauth_email` 回填为 `provider_api_key_name`
   - 返回 `provider_api_key_deleted = true`
4. 其余情况保持当前逻辑不变

### 新增返回字段

在 records 响应中新增：

- `provider_api_key_deleted?: boolean`

语义：

- `true`：当前展示的是“已自动删除账号”的回填显示
- `false` 或空：沿用正常逻辑

## 前端改造

### 位置

- `frontend/src/api/analytics.ts`
- `frontend/src/features/usage/types.ts`
- `frontend/src/features/usage/composables/useUsageData.ts`
- `frontend/src/features/usage/components/UsageRecordsTable.vue`

### 行为

在管理员使用记录表格的“提供商”列第二行：

- 默认继续显示 `record.api_key_name`
- 若 `record.provider_api_key_deleted === true`：
  - 当前这个 `record.api_key_name` 视为账号邮箱展示值
  - 在其后追加一个 `已删除` badge

展示位置不变，只增强语义。

## 回退策略

采用静默回退：

- key 仍存在：正常显示当前 key 名称
- key 已删除但找不到删除日志：保持现状
- 删除日志存在但 `oauth_email` 为空：保持现状

即只有“命中自动删除日志且有邮箱”时才显示账号邮箱与 badge。

## 权限与数据暴露

该增强仅面向管理员 records 视图：

- 现有非管理员 records 响应继续隐藏 provider 相关字段
- 不新增敏感 token 或 auth_config 暴露
- 仅新增一个展示用途的账号邮箱回填

## 测试方案

### 后端测试

新增独立测试文件，覆盖：

1. key 存在时仍返回当前 `provider_api_key_name`
2. key 已删除且命中 `AccessTokenDeleteLog` 时：
   - `provider_api_key_name = oauth_email`
   - `provider_api_key_deleted = true`
3. 没命中删除日志时不误标记
4. 删除日志无邮箱时不误标记

### 前端测试

新增轻量测试，覆盖：

1. records 映射能把 `provider_api_key_deleted` 传入 `UsageRecord`
2. 展示 helper / 表格逻辑在已删除时返回“显示账号 + 已删除 badge”的语义

## 验收标准

1. 自动删除的 access_token 账号在使用记录中，即使 key 已删除，仍可看到对应账号邮箱
2. 该邮箱显示在当前密钥显示位置
3. 该记录后面显示 `已删除` badge
4. 普通未删除 key 的显示逻辑不受影响
5. 普通手动删除 key 不会被误标记为 `已删除`

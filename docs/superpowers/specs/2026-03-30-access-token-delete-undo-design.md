# Access Token 删除账号撤销设计

## 目标

在当前“同一个 access_token-only Codex OAuth 账号连续 3 次真实上游 HTTP 400 才自动删除”的基础上，为今后新删除的账号增加“撤销删除”能力。撤销后的实现语义是：基于删除时保存的快照，新建一个新的 `ProviderAPIKey` 恢复账号，而不是复活原始已删除记录。

## 非目标

- 不支持恢复已经删除的旧记录；旧记录缺少恢复快照，只能查看
- 不改成软删除模型
- 不恢复 Provider，仅恢复被删除的账号（`ProviderAPIKey`）
- 不在前端展示任何敏感快照字段

## 当前约束

现有 `AccessTokenDeleteLog` 只保存展示与排障字段，不保存 `api_key`、`auth_config` 等恢复必需数据，因此当前系统无法对已删除账号做真正恢复。

## 方案选择

采用“扩展现有 `AccessTokenDeleteLog` 保存加密恢复快照”的方案，而不是改成软删除或新增独立归档表。

原因：

- 可复用现有删除历史页与查询接口
- 对现有删除逻辑侵入最小
- 最符合“已经删除，但允许反悔恢复”的语义

## 数据模型变更

扩展 `AccessTokenDeleteLog`，新增两组字段。

### 恢复快照字段

- `snapshot_api_key`
- `snapshot_auth_config`
- `snapshot_allowed_models`
- `snapshot_proxy`
- `snapshot_fingerprint`
- `snapshot_expires_at`
- `snapshot_name`
- `snapshot_is_active`

其中：

- `snapshot_api_key` 与 `snapshot_auth_config` 必须使用现有加密能力存储
- 其余字段用于尽量恢复原账号行为配置

### 恢复状态字段

- `restore_status`
- `restored_key_id`
- `restored_at`
- `restore_error`

状态定义：

- `legacy`：旧版本记录，无恢复快照，只可查看
- `pending`：新版本删除记录，可恢复
- `restored`：已恢复
- `failed`：最近一次恢复失败，但允许再次尝试

## 删除流程改造

当前删除流程保持主语义不变：

1. 同一个 access_token-only key 连续 3 次真实上游 HTTP 400
2. 生成删除日志
3. 删除原始 `ProviderAPIKey`

新增改造：

- 在删除前从当前 key 构造恢复快照
- 将恢复快照一并写入 `AccessTokenDeleteLog`
- 新生成的删除记录默认 `restore_status = pending`
- 旧数据迁移后统一标记为 `legacy`

## 恢复语义

撤销删除定义为：

- 基于删除快照创建一个新的 `ProviderAPIKey`
- 新 key 使用新的主键 ID
- 删除历史中记录 `restored_key_id`
- 恢复后不复活原始 `deleted_key_id`

恢复出来的新 key：

- 保留原 `provider_id`
- 保留原 `auth_type`
- 恢复 `api_key`、`auth_config`、`allowed_models`、`proxy`、`fingerprint`、`expires_at`
- 默认 `is_active = true`
- 清空 auto-delete 的 HTTP 400 连续计数
- 清空 `oauth_invalid_at` / `oauth_invalid_reason`
- 不继承删除前的坏状态计数

## API 设计

新增 admin-only 接口：

- `POST /api/admin/access-token-deletions/{log_id}/restore`

行为：

1. 查询删除记录
2. 校验是否可恢复
3. 根据快照创建新的 `ProviderAPIKey`
4. 更新删除记录：
   - `restore_status = restored`
   - `restored_key_id = 新 key id`
   - `restored_at = now`
   - 清空 `restore_error`
5. 返回恢复后的 key 摘要

失败时：

- 更新 `restore_status = failed`
- 写入 `restore_error`
- 允许再次尝试恢复

现有 list 接口返回项扩展：

- `restore_status`
- `restored_key_id`
- `restored_at`
- `restore_error`
- `can_restore`

## 页面改造

保持当前 Aether 管理页风格，最小改动扩展现有删除历史页。

### 列表页

新增：

- “恢复状态”列

### 详情弹窗

新增展示：

- 恢复状态
- 恢复时间
- 恢复后的 Key ID
- 最近恢复错误

新增动作：

- `撤销删除` 按钮

按钮规则：

- `pending` / `failed`：可点击
- `legacy` / `restored`：不可点击

点击恢复后：

- 调用 restore API
- 成功后刷新 summary 与 list
- 详情状态即时更新

## 错误处理

### 记录不存在

返回 404。

### 旧记录不可恢复

即 `legacy` 记录，返回 409，并明确提示“该记录生成于旧版本，无法恢复”。

### 已恢复记录重复恢复

返回 409。

### 恢复失败

例如：

- provider 不存在
- 快照缺失
- 快照解密失败
- 快照字段非法

行为：

- 更新 `restore_status = failed`
- 写入 `restore_error`
- 允许后续再次尝试

## 安全要求

- 恢复快照只在后端存取
- 前端不得返回或展示 `snapshot_api_key` / `snapshot_auth_config`
- 仅 admin 可执行恢复
- 恢复操作沿用现有加密/解密能力处理敏感字段

## 数据迁移策略

新增 migration：

- 为 `AccessTokenDeleteLog` 添加恢复快照字段与恢复状态字段
- 存量历史记录统一设置：
  - `restore_status = legacy`

这样旧记录在页面中能明确显示“仅可查看，不可恢复”。

## 测试方案

### 后端服务测试

- 删除时写入恢复快照
- 新记录默认 `restore_status = pending`
- restore 成功会创建新的 `ProviderAPIKey`
- restore 成功后删除记录状态更新为 `restored`
- legacy 记录不可恢复
- restore 失败会写 `restore_error`
- 已恢复记录不可重复恢复

### API 测试

- admin 可调用 restore
- 非 admin 被拒绝
- list 返回恢复字段
- restore 成功返回新 key 摘要
- 不可恢复记录返回正确错误码

### 前端测试

- 删除历史页能展示恢复状态
- `pending/failed` 显示“撤销删除”按钮
- `legacy/restored` 不可恢复
- 点击恢复后会刷新状态

## 验收标准

1. 今后新删除的 access_token-only 账号，在删除历史页可见“撤销删除”入口
2. 点击恢复后，系统创建新的 `ProviderAPIKey` 并恢复主要账号配置
3. 恢复后的记录在页面上显示为 `restored`
4. 旧版本删除记录明确显示为 `legacy`，且不能恢复
5. 恢复过程中不会泄露 access token 或 auth config 到前端

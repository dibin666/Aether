# access_token 账号 HTTP 400 自动删除设计

**日期：** 2026-03-30  
**仓库：** `/home/dibin/work/Aether`

## 1. 目标

为 Aether 新增一项自动治理能力：当某个 **Codex / OAuth / 仅有 access_token / 无 refresh_token** 的账号在 **真实上游调用** 中返回 **HTTP 400** 时，系统立即删除这个对应账号（`ProviderAPIKey`），并把删除前的关键信息写入持久化删除历史；同时新增一个保持 Aether 现有后台风格的管理页面，用于展示删除数量和删除明细。

## 2. 边界与非目标

### 2.1 明确边界
- 删除对象仅限 **对应账号（Key）**，不删除 Provider，不影响同 Provider 下其他账号。
- 触发条件仅限：
  1. 这是一次真实上游调用；
  2. 已明确知道当前使用的是哪个具体 Key；
  3. 上游返回 `HTTP 400`；
  4. 该 Key 属于 `Codex + OAuth + access_token-only`。
- 页面风格遵循当前 Aether admin 设计体系，使用现有卡片、筛选栏、表格、详情抽屉模式。

### 2.2 非目标
- 不删除 Provider。
- 不批量删除整组账号。
- 不对 401/403/429/5xx 做同样删除处理。
- 不对有 refresh_token 的完整 OAuth 账号做自动删除。
- 不做恢复/撤销删除功能。
- 不采用截图中的特定视觉样式，仅保留“此类账号触发删除”的语义。

## 3. 方案选择

### 方案 A：请求链路内同步删除 + 独立删除历史表（推荐）
在真实请求失败回收点中识别 `HTTP 400 + access_token-only`，命中后立即：
1. 记录删除历史；
2. 执行 Key 删除；
3. 调用现有删除副作用链。

**优点**
- 满足“立刻删除”的要求；
- 删除对象明确，不需要异步补偿；
- 页面数据可直接读取持久化历史。

**缺点**
- 需要新增数据库表；
- 需要谨慎处理并发重复删除。

### 方案 B：请求链路只打删除事件，由后台异步消费者删除

**缺点**
- 不是严格“立刻删除”；
- 会引入待删除中间态；
- 复杂度更高。

### 方案 C：仅软删除/停用 Key

**缺点**
- 不符合“立刻删除账号”的明确要求。

**结论：**采用 **方案 A**。

## 4. 总体架构

新增四个单元：

1. **删除判定器**
   - 挂在真实上游调用失败链路；
   - 负责识别“当前 Key 是否满足 access_token-only 且 status=400”。

2. **同步删除执行器**
   - 负责删除 `ProviderAPIKey`；
   - 复用现有 Key 删除副作用与引用清理链。

3. **删除历史表**
   - 在删除前保存关键快照；
   - 用于页面展示与排障追溯。

4. **Admin 管理页面**
   - 提供删除数量统计、筛选、列表和详情抽屉；
   - 样式与当前 Aether 后台保持一致。

## 5. 触发语义

只有满足以下全部条件才会删除：
- 这次调用确实发到了上游；
- 当前选中的资源是一个具体 `ProviderAPIKey`；
- 上游响应 `HTTP 400`；
- `provider_type == codex`；
- `auth_type == oauth`；
- `auth_config` 中没有 `refresh_token`；
- 当前 Key 仍存在。

### 不触发删除的情况
- 本地参数校验错误；
- 请求未真正发送到上游；
- 上游不是 400；
- 账号有 refresh_token；
- 非 Codex Provider；
- 非 OAuth Key；
- Key 已被别的请求先删除。

## 6. 删除历史表设计

建议新增表：`access_token_delete_logs`

字段建议：
- `id`
- `deleted_key_id`
- `provider_id`
- `key_name`
- `oauth_email`
- `provider_type`
- `auth_type`
- `trigger_status_code`
- `endpoint_sig`
- `proxy_node_id`
- `proxy_node_name`
- `request_id`
- `error_message`
- `raw_error_excerpt`
- `deleted_at`
- `deleted_by`（固定：`system:auto-delete-http400`）

### 设计原则
- 只保存展示与排障必要字段，不保存整份大请求体。
- 即使原 Key 已被删除，日志仍保留完整快照。
- 对 `deleted_key_id` 建议增加唯一约束，避免并发重复插入同一删除记录。

## 7. 删除执行顺序

推荐顺序：
1. 根据 `key_id` 再次查询并确认 Key 仍存在；
2. 组装删除快照；
3. 写入删除历史；
4. 执行 Key 删除副作用；
5. 删除 `ProviderAPIKey`；
6. 提交事务。

这样可确保“先记账、后删实体”，删除后仍能保留可追溯记录。

## 8. 并发与幂等

为避免同一个 Key 因多个并发 400 被重复删除，使用两层保护：

### 8.1 事务内存在性确认
删除前按 `key_id` 再次查询：
- 如果不存在，说明已被其他请求删除，当前流程直接跳过。

### 8.2 删除历史唯一约束
对 `deleted_key_id` 施加唯一约束：
- 第一个请求写入成功并删 Key；
- 后续重复请求插入日志时命中唯一约束或查不到 Key，则幂等结束。

## 9. 页面设计

### 9.1 路由与菜单
新增 admin 页面：
- 路由：`/admin/access-token-deletions`
- 菜单名称：`删除账号历史`（可在实现时微调文案）

### 9.2 页面结构
使用当前 Aether admin 常见布局：

#### A. 顶部统计卡
展示：
- 总删除数
- 今日删除数
- 最近 24 小时删除数

#### B. 筛选栏
支持：
- 邮箱搜索
- 最近 N 天
- Provider 过滤（可选）

#### C. 删除历史表格
列建议：
- 删除时间
- 邮箱
- Key 名称
- Provider
- 格式
- 代理节点
- 状态码
- 错误摘要

#### D. 详情抽屉 / Modal
点选一行后展示：
- `deleted_key_id`
- `request_id`
- `endpoint_sig`
- `proxy_node_id / proxy_node_name`
- 完整错误摘要
- `raw_error_excerpt`

## 10. API 设计

新增 admin-only API：

### `GET /api/admin/access-token-deletions/summary`
返回：
- 总删除数
- 今日删除数
- 最近 24h 删除数

### `GET /api/admin/access-token-deletions`
返回分页列表，支持筛选：
- `email`
- `provider_id`
- `days`
- `limit`
- `offset`

本期不提供恢复接口。

## 11. 后端代码落点

### 请求删除逻辑
- 在真实请求失败回收点（已知具体 key 和 status code 的位置）挂接自动删除判定；
- 不应放在过高层的纯通用异常包装层，以免误删未命中具体 key 的情况。

### 新增模块建议
- `src/services/provider_keys/access_token_auto_delete.py`
  - 判定 access_token-only
  - 记录删除历史
  - 执行同步删除

- `src/api/admin/access_token_deletions.py`
  - summary/list 接口

- `src/models/database.py`
  - 新增删除历史 ORM 模型

- `src/main.py` / admin 路由注册点
  - 注册新 API

## 12. 错误处理

### 12.1 删除记录写入失败
- 不应静默吞掉；
- 若写历史失败，则不执行实体删除，避免出现“账号被删但无记录”。

### 12.2 删除副作用失败
- 记录错误并回滚事务；
- Key 保持存在，避免系统进入半删除状态。

### 12.3 重复删除
- 若 Key 不存在或日志已存在，视为幂等成功，不抛给用户。

### 12.4 页面查询失败
- 返回标准 admin 错误响应；
- 前端按现有页面风格展示错误态。

## 13. 测试方案

### 13.1 后端单测
覆盖：
- `HTTP 400 + access_token-only` 会删除 Key；
- 非 400 不删；
- 有 refresh_token 不删；
- 非 codex 不删；
- 删除前会写删除历史；
- 重复删除幂等跳过。

### 13.2 API 测试
覆盖：
- `summary` 返回统计；
- `list` 返回分页与筛选结果；
- 非 admin 请求被拒绝。

### 13.3 前端测试
覆盖：
- 新路由存在；
- 菜单项可见；
- 页面能渲染统计卡、筛选栏和表格；
- 点击行能打开详情抽屉；
- 筛选参数正确传递给 API。

## 14. 验收标准

满足以下条件即可视为完成：
1. 某个 `Codex + OAuth + access_token-only` 账号在真实上游调用中返回 `HTTP 400` 时，会被立即删除；
2. 删除仅影响该 Key，不影响 Provider 和其他账号；
3. 删除前会写入一条持久化删除历史；
4. 管理员可在新页面查看删除总数、今日数量、最近 24h 数量与删除明细；
5. 页面风格与当前 Aether admin 保持一致；
6. 重复命中同一个已删账号时不会产生重复删除或重复日志。

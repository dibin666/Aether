# Access Token 删除前活跃请求保护设计

## 目标

优化 access_token-only 账号的自动删除逻辑：当同一个 key 连续 3 次真实上游 HTTP 400 达到删除阈值时，如果该账号还有其他正在执行中的请求，则本次先跳过删除；等下一次再命中 400 且已无其他活跃请求时，再执行删除。

## 核心语义

- 连续 400 计数逻辑保持不变
- 当计数达到删除阈值时，删除前新增一次“同 key 其他活跃请求”检查
- 若存在其他活跃请求：
  - 本次不删除
  - 不写删除历史
  - 保留当前连续 400 计数
- 若不存在其他活跃请求：
  - 立即按现有流程删除
  - 写删除历史

## 关键约束

必须排除“当前这次刚返回 400 的请求自己”。

否则当前请求在状态尚未完全收尾时，也会被算作活跃请求，导致系统每次都检测到“仍有活跃请求”，最终永远不删除该账号。

## 方案选择

采用“删除前直接查询 `Usage` 活跃状态”的方案，而不是新增内存级 in-flight 计数器。

原因：

- 复用现有运行态请求数据
- 不引入多进程内存一致性问题
- 对现有删除链路改动最小
- 重启后不会丢失语义

## 数据来源

使用 `Usage` 表中的请求状态作为活跃请求判定来源：

- `provider_api_key_id`
- `request_id`
- `status`

活跃请求状态定义为：

- `pending`
- `streaming`

## 判定规则

给定当前触发 400 的 key 与 request：

- 同 key：`Usage.provider_api_key_id == key_id`
- 活跃状态：`Usage.status in ('pending', 'streaming')`
- 排除当前请求：`Usage.request_id != current_request_id`

如果存在满足以上条件的记录，则认为“该账号仍有其他活跃请求”。

## 代码落点

### `src/services/provider_keys/access_token_auto_delete.py`

新增 helper：

- `_has_other_inflight_requests_for_key(db, key_id, current_request_id)`

职责：

- 查询 `Usage`
- 判断当前 key 是否存在其他活跃请求

改造：

- `delete_access_token_only_key_on_http400(...)`

新流程：

1. 校验 status_code=400、key 存在且属于 access_token-only OAuth key
2. 增加连续 400 计数
3. 若未达到阈值，直接返回 `False`
4. 若达到阈值，先检查是否仍有其他活跃请求
5. 有其他活跃请求则直接返回 `False`
6. 无其他活跃请求时才继续执行现有删除流程

### `src/services/orchestration/error_handler.py`

不改变整体调用链，仅继续将：

- `key_id`
- `request_id`

透传给 `delete_access_token_only_key_on_http400(...)`。

## 行为细节

### 保留计数，不回退

当达到阈值但因为存在其他活跃请求而跳过删除时：

- 保留当前 400 连续计数
- 不清零
- 不降回 2

这样下次同 key 再出现 400 且没有其他活跃请求时，可以立即删除。

### request_id 缺失时的保守策略

若未来某些路径未提供 `request_id`：

- 无法准确排除当前请求
- 系统应采用保守策略：只要检测到该 key 有活跃请求，就跳过本次删除

这会让删除更晚发生，但不会误删仍在处理中的账号。

## 测试方案

### 服务层测试

在 `tests/services/test_access_token_auto_delete.py` 中新增：

1. 达到阈值且无其他活跃请求时正常删除
2. 达到阈值但有其他活跃请求时跳过删除
3. 只有当前 request 自己仍活跃时，因被排除，仍允许删除

### 错误处理层测试

保留 `tests/services/test_error_handler_access_token_delete.py` 现有覆盖，确保：

- `request_id` 仍正确透传给删除服务

## 验收标准

1. 同 key 连续 3 次真实上游 HTTP 400 的删除行为仍然保留
2. 若达到删除阈值时该 key 仍有其他活跃请求，本次不删除
3. 当这些活跃请求结束后，下次同 key 再命中 400 时可立即删除
4. 当前触发 400 的请求自身不会阻止删除判定
5. 不新增前端配置项，不改删除历史页交互

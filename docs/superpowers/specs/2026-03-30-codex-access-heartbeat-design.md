# Codex Access Token 心跳页设计

**日期：** 2026-03-30  
**仓库：** `/home/dibin/work/Aether`

## 1. 目标

为 Aether 新增一个 `Codex 心跳` 管理页面，用于对 **Codex + OAuth + 仅 access_token + 无 refresh_token** 的账号执行常驻心跳。心跳行为为：**每个账号在上一轮请求完成后等待 2 秒，再向 `gpt-5.2` 发送一个非流最小请求**。页面提供全局启动/停止与状态观测能力。

## 2. 背景与约束

### 2.1 背景
- 当前 Aether 已支持将 access_token-only 的 Codex OAuth 账号导入到 Provider OAuth。
- 这类账号缺少 refresh_token，无法依赖常规刷新链路维持活跃状态。
- 需要一个后台机制持续对这些账号发送轻量请求，以实现 keepalive/heartbeat。

### 2.2 明确约束
- **仅处理目标账号：** `provider_type=codex`、`auth_type=oauth`、有 access_token、无 refresh_token、`is_active=true`。
- **仅提供全局启停：** 本期不做逐账号启停。
- **不自动落库运行态：** 全部运行状态保存在内存，服务重启后丢失可接受。
- **不自动开机恢复：** 服务重启后默认关闭，需管理员手动启动。
- **不治理账号：** 心跳失败只更新运行态，不自动停用 key、不删除 key、不写 oauth_invalid 标记。
- **非流请求：** 固定发送非流最小负载，不复用 quota refresh 接口。

## 3. 方案选择

### 方案 A：新 admin 页面 + 后端常驻 Heartbeat Manager（推荐）
- 新增独立 admin 页面和后端服务。
- 页面通过 start/stop/status API 控制一个常驻 manager。
- manager 为每个符合条件的账号维护独立 async loop。

**优点：**
- 满足“每账号每 2 秒”的高频要求。
- 页面关闭不影响任务。
- 状态清晰，适合管理和排障。

**缺点：**
- 需要新增长生命周期服务与状态管理。
- 高频任务需要控制好生命周期与错误隔离。

### 方案 B：页面轮询驱动心跳
- 仅页面打开时前端定时触发心跳。

**缺点：**
- 浏览器关闭后任务消失。
- 不是真正的后台常驻任务。
- 不符合目标。

### 方案 C：复用 maintenance scheduler 批处理轮询
- 定时扫描账号并批量发送心跳。

**缺点：**
- 难以精确实现“每账号每 2 秒”。
- 账号增多时节奏会漂移。

**结论：**采用 **方案 A**。

## 4. 总体架构

新增 4 个核心单元：

1. **Admin 页面**
   - 路由：`/admin/codex-heartbeat`
   - 功能：全局启动/停止、展示全局统计与账号状态表。

2. **CodexAccessHeartbeatManager**
   - 进程内单例。
   - 负责启动/停止、定期扫描目标账号、为每个账号维护 worker。

3. **单账号 Heartbeat Worker**
   - 只负责一个 key 的循环请求。
   - 严格保证同一账号同一时刻仅有一个 in-flight 请求。

4. **请求执行器**
   - 使用该 key 的 access_token 向 OpenAI/Codex 发送固定非流请求。
   - 返回 success/failure、耗时和错误信息。

## 5. 页面设计

### 5.1 路由与菜单
- 新增 admin 路由：`/admin/codex-heartbeat`
- 在 `MainLayout` 管理导航下新增菜单项：`Codex 心跳`

### 5.2 页面布局
页面由两部分组成：

#### A. 全局控制卡片
展示：
- 当前状态：`运行中 / 已停止`
- 启动时间
- 管理账号数
- 活跃 worker 数
- 总成功次数
- 总失败次数
- 最近全局错误

按钮：
- `启动心跳`
- `停止心跳`

#### B. 账号状态表
每行展示：
- `key_id`
- `email`
- `provider`
- `expires_at`
- `最近心跳时间`
- `最近成功时间`
- `最近失败时间`
- `最近状态`
- `连续失败次数`
- `累计成功`
- `累计失败`
- `最近耗时(ms)`
- `最近错误`

## 6. 后端状态模型

### 6.1 全局状态
```python
{
  "running": bool,
  "started_at": datetime | None,
  "managed_accounts": int,
  "active_workers": int,
  "total_success": int,
  "total_failed": int,
  "last_error": str | None,
}
```

### 6.2 单账号状态
```python
{
  "key_id": str,
  "provider_id": str,
  "email": str | None,
  "expires_at": int | None,
  "running": bool,
  "last_heartbeat_at": datetime | None,
  "last_success_at": datetime | None,
  "last_failure_at": datetime | None,
  "last_latency_ms": int | None,
  "consecutive_failures": int,
  "success_count": int,
  "failure_count": int,
  "last_error": str | None,
  "last_status": "success" | "error" | "paused",
}
```

### 6.3 存储策略
- 所有状态保存在 manager 内存中。
- API 读取 manager 快照返回给前端。
- 不写数据库。

## 7. 目标账号筛选规则

manager 扫描到的 key 必须同时满足：
- `provider.type == codex`
- `key.auth_type == oauth`
- `key.is_active == true`
- `key.api_key` 可解密为非空 access_token
- `auth_config.refresh_token` 为空或不存在

默认排除：
- 非 Codex provider
- 非 OAuth key
- 已停用 key
- 有 refresh_token 的完整 OAuth key
- provider/key 已删除或无效

## 8. 运行语义

### 8.1 全局生命周期
- `start()`：启动 manager，若已运行则幂等返回当前状态。
- `stop()`：停止全部 worker，清理运行态。
- 页面关闭不影响 manager。
- 服务重启后默认关闭，不自动恢复。

### 8.2 动态发现
manager 在运行期间定时重扫 eligible keys（建议每 10 秒一次）：
- 新导入的 access_token-only key 自动加入心跳。
- 已变更为不符合条件的 key 自动移除 worker。

### 8.3 单账号循环
单账号 worker 循环逻辑：
1. 发送一次心跳请求。
2. 更新单账号状态。
3. 等待 2 秒。
4. 继续下一轮。

**注意：**2 秒的语义为“上一轮请求完成后再等待 2 秒”，不使用固定 wall-clock 节拍，避免同一账号请求重叠。

## 9. 心跳请求设计

### 9.1 固定请求内容
```json
{
  "model": "gpt-5.2",
  "messages": [
    {"role": "user", "content": "ping"}
  ],
  "stream": false,
  "temperature": 0,
  "max_tokens": 1
}
```

### 9.2 设计原则
- 使用最小负载，减少成本和时延。
- 强制非流，简化 worker 状态机。
- 固定模型为 `gpt-5.2`。

### 9.3 请求链路
- 不走 quota refresh。
- 不让页面直接调用公共负载均衡入口。
- 由后端请求执行器按 **具体 key** 使用该 access_token 发请求，确保真正使用到目标账号。
- 尽量复用现有 OpenAI/Codex 请求构造与 HTTP 客户端约定，而非全新绕过实现。

## 10. API 设计

新增 admin-only API：

### `GET /api/admin/codex-heartbeat/status`
返回：
- 全局状态
- 单账号状态列表

### `POST /api/admin/codex-heartbeat/start`
行为：
- 启动 manager
- 已运行时幂等返回当前状态

### `POST /api/admin/codex-heartbeat/stop`
行为：
- 停止 manager
- 返回停止后的状态

## 11. 错误处理

### 11.1 单账号请求失败
- 更新 `last_failure_at`
- `failure_count + 1`
- `consecutive_failures + 1`
- 写入 `last_error`
- 不影响其他账号 worker

### 11.2 单账号请求成功
- 更新 `last_success_at`
- `success_count + 1`
- `consecutive_failures = 0`
- 清空 `last_error`

### 11.3 全局错误
- manager 扫描异常或启动异常时更新 `last_error`
- 不导致整个服务崩溃；必要时停止 manager 并将状态暴露给页面

### 11.4 不做的动作
本期明确不做：
- 自动停用 key
- 自动删除 key
- 自动写 `oauth_invalid_at/reason`
- 自动触发 quota refresh

## 12. 文件落点

### 后端
- `src/api/admin/codex_heartbeat.py`
  - 新增 admin API：start / stop / status

- `src/services/provider_keys/codex_access_heartbeat.py`
  - Heartbeat manager
  - 账号扫描
  - worker 生命周期
  - 请求执行器
  - 状态快照

- `src/main.py`
  - 应用关闭时停止 heartbeat manager
  - 不在启动时自动启动

### 前端
- `frontend/src/api/endpoints/codex_heartbeat.ts`
  - API 封装

- `frontend/src/views/admin/CodexHeartbeat.vue`
  - 新管理页

- `frontend/src/router/index.ts`
  - 新增 admin 路由

- `frontend/src/layouts/MainLayout.vue`
  - 新增侧边栏菜单

## 13. 测试方案

### 13.1 后端单测
覆盖：
- 正确筛出 access_token-only 的 Codex OAuth keys
- `start()` 启动成功且幂等
- `stop()` 停止成功且幂等
- 单账号成功请求会更新 success 状态
- 单账号失败请求会更新 failure 状态
- 重扫后新账号会自动加入，失效账号会自动移除

### 13.2 API 测试
覆盖：
- `status` 返回全局状态和行数据
- `start` / `stop` 对非 admin 拒绝访问
- `start` / `stop` 返回结构稳定

### 13.3 前端测试
覆盖：
- 新路由存在
- 菜单项可见
- 页面能渲染全局状态卡片和账号表格
- 启动/停止按钮触发 API 调用

## 14. 分阶段实现建议

### Phase 1（本次）
- 后台常驻 manager
- admin 页面
- start / stop / status API
- 每账号 2 秒非流请求
- 内存状态表

### Phase 2（未来可选）
- 逐账号启停
- 自动开机恢复
- 参数配置（模型、间隔、超时）
- 持久化运行态
- 更丰富的告警/治理动作

## 15. 验收标准

满足以下条件即可视为完成：
1. 管理员可在 `/admin/codex-heartbeat` 页面手动启动/停止心跳。
2. 仅 `Codex + OAuth + access_token-only` 账号会进入心跳池。
3. 每个账号在请求完成后等待 2 秒，再发送一次新的非流 `gpt-5.2` 请求。
4. 页面能实时看到全局状态和每个账号最近成功/失败情况。
5. 停止后全部 worker 能退出，不再继续发请求。
6. 服务关闭时 heartbeat manager 能被正确停止。

## 16. 非目标

本设计不包含：
- 逐账号启停
- 页面 websocket 实时推送
- 心跳参数自定义
- 自动修复/自动禁用异常账号
- 启动后自动恢复历史运行状态

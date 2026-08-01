# ADR-003 · 路由级 upstream 超时覆盖

- **状态**：Accepted
- **日期**：2026-08-01
- **影响**：`api-gateway`, `libs/common-http`
- **关联**：[路由配置手册](../guides/api-gateway-routing#25-per-route-upstream-超时覆盖)

## 上下文

api-gateway 全局 `ResilienceConfig.timeout_ms` 默认 3000ms。
当后端（如 `pm-platform`）调 LLM 接口（DeepSeek / MiniMax 同步响应 5-40s）
时，3s 后网关切断连接并返回 504 Gateway Timeout。

1 月 1 日起下游 `project-manager` 在 `POST /pm-api/v1/ai/parse-requirement`
上频繁复现（314 并发请求中的第 0 个，57.5KB 多文档预览需求文本）。
日志证据：`09:51:07` gateway 收到请求 → `09:51:10` `forward to pm-platform timed out`
→ `09:51:14` pm-platform 后端才完成写库（LLM 调了 7s）。

## 决策

给 `RouteConfig` 增加可选字段 `timeout_ms: Option<u64>`（毫秒），
转发时调 `ResilientClient::call_with_timeout()` 覆盖单次发送与整体超时；
缺省 `None` 走全局 `cfg.resilience` 默认值。

### 为什么 per-route

- **全局放宽**（如 `timeout_ms = 90000` 全局生效）会掩盖其他服务的真实故障
- **单 route 配置精准**：AI 路由 90s，其他路由（IAM / audit / file）仍保持 3s
- **向后兼容**：未配置字段的现有路由行为完全不变

### 为什么 override 时 per_attempt_timeout == overall_timeout

`should_retry_err(retry.rs:45)` 对 POST 返回 false（POST 非幂等）。
→ 单次 timeout 触发后立即返回 `ResilientError::Timeout`，不进入重试循环。
→ 把 per_attempt 和 overall 都设为传入的超时值是安全的：overall 的 8s 默认值不会截断 90s 单次调用。

对幂等方法（GET / PUT / DELETE）此语义同样安全——per-route override 不影响
`retry_max_attempts`，重试次数仍由全局 cfg 控制。

### 实现

| 文件 | 改动 |
|------|------|
| `libs/common-http/src/resilient.rs` | 抽 `call_inner` / `call_streaming_inner` 私有 helper 接受 `timeout_opt: Option<Duration>`；新增 public `call_with_timeout` / `call_streaming_with_timeout` |
| `services/api-gateway/src/config.rs` | `RouteConfig.timeout_ms: Option<u64>` 字段（serde default None） |
| `services/api-gateway/src/router.rs` | `forward_to_service` / `forward_streaming` 接受 `timeout_ms` 参数，match 分流 call / call_with_timeout |
| `services/api-gateway/src/main.rs` | `proxy_handler` 透传 `route.timeout_ms` |

## 取舍

- 路由文件复杂度上升（需要注释解释 `timeout_ms` 字段和字面量长度排序陷阱）
- 镜像版本硬依赖：旧镜像 v1.0.0 解析 unknown field `timeout_ms` 会失败
  → **必须先升级镜像再 reload 含新字段的路由**
- 单元测试 4 条 `call_with_timeout` / 2 条 `RouteConfig` 解析（向后兼容验证）

## 替代方案

### 全局放宽（否决）

- 把 `[resilience] timeout_ms = 90000` 配进 `api-gateway.toml`
- 否决理由：所有 upstream 都放宽，其他 3s 就应暴露的故障会被掩盖

### SSE 流式改造（暂搁）

- 把 `parse-requirement` 改成 SSE（`text/event-stream`）透传 LLM bytes
- 理由：超时放宽足以覆盖当前 sync LLM 调用（同步返回 4KB JSON，40s 内完成）
- SSE 在需要「on-the-fly 摘 token + 进度条 UI」的迭代中可后续引入，不在此 scope

### 直连 pm-platform 绕过 gateway（不可控）

- 前端 vite proxy 改 `target` 直连 pm-platform → 绕过 api-gateway
- 否决理由：脱钩统一鉴权/JWT/审计/限流，不适合生产

## 后果

- AI 拆解端点 504 的根因消除
- 各路由时间窗口自治性提升
- 路由配置的审查复杂度轻微增加（新增字段 + 排序陷阱）
- 其他 contributor 可通过 `ADR-003` + 路由手册 §2.5 自主配新长超时路由
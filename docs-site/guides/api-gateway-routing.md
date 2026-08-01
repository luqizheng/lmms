# API Gateway 路由配置手册

适用版本：`api-gateway` 当前实现（多文件路由 + 热重载）。
决策依据见 [ADR-001](../decisions/ADR-001-api-gateway-multi-file-router-config.md)。

---

## 1. 文件布局

```
infrastructure/config/api-gateway/
├── api-gateway.toml          # 全局：server / redis / jwks / consul
├── rate-limits.toml          # 限流策略
└── routes/
    ├── audit.router.toml
    ├── auth.router.toml
    ├── iam.router.toml
    ├── notification.router.toml
    ├── permissions.router.toml
    ├── project.router.toml
    ├── prue_backend.router.toml
    ├── roles.router.toml
    ├── token.router.toml
    └── user.router.toml
```

> Phase 9: `project.router.toml` is the Spring Boot service route — see
```

- `routes/` 下所有匹配 `*.router.toml` 的文件会被加载
- 文件名按字典序排序合并，**每个前缀只能有一条路由**
- `api-gateway.toml` 中不再有 `[[routes]]` 块

---

## 2. 路由字段

```toml
[[route]]
match    = "/users/**"            # 路径匹配模板（必填）
service  = "iam-service"          # Consul 服务名（必填）
public   = false                  # 是否跳过 JWT（默认 false）
rewrite  = [ ... ]                # 重写规则链（可选）
```

| 字段 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `match` | string | 是 | 精确路径或 `/prefix/**` 通配 |
| `service` | string | 是 | Consul 中注册的服务名 |
| `public` | bool | 否 | `true` 跳过 JWT 鉴权，默认 `false` |
| `rewrite` | array | 否 | 顺序执行的规则链，见 §4 |
| `timeout_ms` | u64 | 否 | 可选 per-route 单次请求超时（毫秒）。`Some(ms)` 时覆盖全局 `resilience.timeout_ms`（默认 3000）。适用 LLM / 长 RPC 等需要 >3s 的端点。详见 §2.5。 |

---

## 2.5 Per-route upstream 超时覆盖

适用场景：调 LLM / 长 RPC / 慢报表等下游端点。背景：全局 `resilience.timeout_ms` 默认 3000ms，长耗时调用必然 504。决策与权衡详见 [ADR-003](../decisions/ADR-003-route-timeout-override)。

### 示例：AI 拆解端点 90s 超时

```toml
# pm.router.toml — 下游 pm-platform 的 AI 拆解路由
# upstream/downstream 字面量长度降序匹配，/pm-api/v1/ai/{rest} 优先于通用 /pm-api/v1/{rest}
[[route]]
upstream    = "/pm-api/v1/ai/{rest}"
downstream  = "/pm-api/v1/ai/{rest}"
service     = "pm-platform"
timeout_ms  = 90000
```

### 路由匹配陷阱

`routes_loader.rs` 按 `upstream` 字面量长度**降序**排序匹配。
带 `timeout_ms` 的更具体的路由**必须**字面量长度 > 通用路由（否则通用 route 先命中且无 `timeout_ms`，3s 超时立刻生效）。

### 镜像版本要求

字段 `timeout_ms` 引入于 api-gateway `v1.1.0`。`v1.0.0` 镜像启动时会因 unknown field 解析失败。**必须先升级镜像再 reload 含新字段的路由**。

### 设计 rationale

`POST` 请求非幂等（`should_retry_err` 对 POST 返回 false）→ 网关不重试。
`call_with_timeout()` 内 per-attempt 与 overall deadline 同步放宽：
长时间调用一次就够，不怕 overall 8s 截断 90s 单次。
对其他幂等方法（GET/PUT/DELETE），per-route override 仍安全，retry 次数取决于 `retry_max_attempts`。

---

## 3. match 模式

### 精确匹配

```toml
match = "/health"
```

只匹配 `/health`。

### 前缀通配（最常用）

```toml
match = "/v1/iam/users/**"
```

匹配 `/v1/iam/users`、`/v1/iam/users/1`、`/v1/iam/users/1/profile`。

### 多条前缀 → 多个文件

```toml
# user.router.toml
[[route]]
match   = "/v1/iam/users/**"
service = "iam-service"

# roles.router.toml
[[route]]
match   = "/v1/iam/roles/**"
service = "iam-service"
```

### 冲突检测

两个文件声明相同 `match` → 启动 / 重载时报错：

```
duplicate match pattern: /v1/iam/users/**
```

---

## 4. rewrite 规则链

每条路由可声明 `rewrite = [...]`，**按数组顺序执行**。
`redirect` 是终结规则，触发后直接返回 30x，不再向下游转发。

### 4.1 strip_prefix — 去掉路径前缀

请求 `/api/v1/iam/users/1` → 后端收到 `/v1/iam/users/1`：

```toml
[[route]]
match   = "/api/v1/iam/users/**"
service = "iam-service"
rewrite = [
  { type = "strip_prefix", prefix = "/api" }
]
```

### 4.2 redirect — 重定向（终结）

```toml
[[route]]
match   = "/old-path"
service = "iam-service"
rewrite = [
  { type = "redirect", status = 301, to = "/new-path" }
]
```

支持 `301` / `302` / `307` / `308`。

### 4.3 header_set — 覆盖请求头

```toml
[[route]]
match   = "/v1/iam/users/**"
service = "iam-service"
rewrite = [
  { type = "header_set", name = "X-Source", value = "gateway" }
]
```

### 4.4 header_add — 追加请求头

```toml
[[route]]
match   = "/v1/iam/users/**"
service = "iam-service"
rewrite = [
  { type = "header_add", name = "X-Trace", value = "on" }
]
```

### 4.5 path_param — 路径片段重写

把 `/users/{id}/profile` 重写为 `/profile?u={id}`：

```toml
[[route]]
match   = "/users/**"
service = "iam-service"
rewrite = [
  { type = "path_param", from = "/users/", to = "/profile?u=" }
]
```

### 4.6 组合示例

带审计 header、剥前缀、声明为公开路由：

```toml
[[route]]
match   = "/api/v1/auth/logout"
service = "iam-service"
public  = true
rewrite = [
  { type = "header_set", name = "X-Auth-Audit", value = "logout" },
  { type = "strip_prefix", prefix = "/api" }
]
```

---

## 5. CLI 子命令

### 5.1 validate — 仅校验（CI 友好）

```bash
cargo run -p api-gateway -- validate \
  --routes infrastructure/config/api-gateway/routes
```

- 解析所有 `*.router.toml`
- 完整校验（match 唯一、service 非空、rewrite 类型合法）
- **退出码 0** = 通过；**退出码 1** = 失败，stderr 列出原因
- 不启动 gateway，不触达运行时

### 5.2 serve — 启动网关

```bash
cargo run -p api-gateway -- serve \
  --routes infrastructure/config/api-gateway/routes \
  --admin-port 9090
```

| 参数 | 默认 | 说明 |
|------|------|------|
| `--routes` | `infrastructure/config/api-gateway/routes` | 路由目录 |
| `--admin-port` | `9090` | admin HTTP 端口；设为 `0` 禁用 |

启动日志打印全部路由与 rewrite 数量。

---

## 6. 热重载

配置改完后**两种**触发方式，二选一：

### 6.1 SIGHUP（POSIX：Linux / macOS / WSL）

```bash
# 找出 gateway 进程
pid=$(pgrep -f "api-gateway serve")
kill -HUP "$pid"
```

gateway 重新解析路由，校验失败时**保留旧表**，错误写到日志：

```
ERROR SIGHUP reload failed: parse user.router.toml: ...
```

### 6.2 HTTP admin endpoint（Windows + 跨平台）

```bash
curl -X POST http://localhost:9090/__admin/reload
```

- 成功：`200 OK`，body `{"reloaded": 9}`
- 失败：`500`，body `{"error": "..."}`，旧表保持不变

### 6.3 推荐工作流

```bash
# 1. 改完路由后先 validate
cargo run -p api-gateway -- validate \
  --routes infrastructure/config/api-gateway/routes

# 2. 通过后 reload
curl -X POST http://localhost:9090/__admin/reload
```

---

## 7. 排错清单

| 现象 | 原因 | 处理 |
|------|------|------|
| 启动 panic：`failed to load routes from ...` | routes/ 为空或权限不足 | 至少放一个 `*.router.toml`，确认读权限 |
| `duplicate match pattern: /v1/iam/users/**` | 两个文件用了相同 `match` | 合并到一个文件或换前缀 |
| `parse auth.router.toml: unknown variant unknown_op` | rewrite 类型拼错 | 限定为 `strip_prefix` / `redirect` / `header_set` / `header_add` / `path_param` |
| reload HTTP 500 | 新配置不合法 | gateway 保留旧表；fix 配置后再次 reload |
| Windows 上 reload 没反应 | 不支持 SIGHUP | 用 `POST /__admin/reload` |
| `prefix not in path` 出现在请求日志 | strip_prefix 配置的 prefix 不在路径里 | 改为精确匹配或调前缀 |
| AI / LLM 端点返回 504，但下游日志显示处理了 5s+ | 该路由缺 `timeout_ms` 配置，被 3s 全局超时截断 | 在对应 `*.router.toml` 加 `timeout_ms = 90000`（或更长），确认镜像 ≥ v1.1.0 后 reload |

---

## 8. 与 Consul 的关系

- `service` 字段 = Consul service name，由 `services/<svc>` 在启动时向 Consul 注册
- 路由命中后，gateway 调 Consul `/v1/health/service/<svc>` 拿地址，转发请求
- Consul 仅承担**服务发现**，**不**分发路由配置 —— 路由文件随部署一起走

---

## 9. 参考

- 决策记录：[`docs-site/decisions/ADR-001-api-gateway-multi-file-router-config.md`](../decisions/ADR-001-api-gateway-multi-file-router-config.md)
- 灰度发布与流量分配：[`docs-site/guides/canary-rollout.md`](canary-rollout.md)
- 限流配置：`infrastructure/config/api-gateway/rate-limits.toml`
- 网关源码：`services/api-gateway/src/`

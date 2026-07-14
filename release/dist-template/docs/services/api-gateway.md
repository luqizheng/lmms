# API 网关

API 网关是系统的统一入口，负责限流、鉴权、路由分发。

## 端口

| 端口 | 说明 |
|------|------|
| 3000 | HTTP 入口（默认）|

## 核心配置

`config/api-gateway/api-gateway.toml` 中配置：

```toml
[server]
host = "0.0.0.0"
port = 3000

[redis]
url = "redis://redis:6379"

[consul]
addr = "http://consul:8500"

[jwt]
jwks_url = "http://token-service:8082/.well-known/jwks.json"
issuer = "superpowers-token-service"
```

## 路由规则

路由配置在 `config/api-gateway/routes/` 下，按 URL 前缀分流到各后端服务：

| 文件 | 匹配前缀 | 目标服务 |
|------|---------|---------|
| `auth.router.toml` | `/api/v1/auth` | iam-service |
| `user.router.toml` | `/api/v1/users` | iam-service |
| `roles.router.toml` | `/api/v1/roles` | iam-service |
| `permissions.router.toml` | `/api/v1/permissions` | iam-service |
| `config.router.toml` | `/api/v1/config` | config-service |
| `file.router.toml` | `/api/v1/file` | file-service |
| `notification.router.toml` | `/api/v1/notification` | notification-service |
| `audit.router.toml` | `/api/v1/audit` | audit-service |
| `prue_backend.router.toml` | `/api/v1/prue/*` | prue_backend |

## 限流

限流规则在 `config/api-gateway/rate-limits.toml`：

```toml
[defaults]
requests_per_second = 100
burst = 200

[routes."/v1/iam/auth/login"]
requests_per_second = 5
burst = 10
```

限流时返回 HTTP 429 + `Retry-After: 1`。

## 健康检查

```bash
curl http://localhost:3000/health
```

## 日志级别

通过环境变量调整：

```env
RUST_LOG=api_gateway=trace,info
```

- `trace`：完整请求/响应日志
- `info`：仅关键事件

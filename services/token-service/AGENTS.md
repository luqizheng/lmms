# token-service 服务规范

## 必需组件

- [x] Dockerfile
- [x] AGENTS.md
- [x] README.md
- [x] tests/ 目录
- [x] /health 端点
- [x] /metrics 端点
- [x] migrations/ 目录
- [x] Swagger 配置（SWAGGER_ENABLED 开关）

## 环境变量

| 变量 | 默认值 | 说明 |
|------|--------|------|
| SERVER_HOST | 0.0.0.0 | 监听主机 |
| SERVER_PORT | 8080 | 监听端口 |
| SERVER_ADDR | 0.0.0.0:8080 | 完整监听地址 |
| DATABASE_URL | 服务自定义 | MySQL 连接 |
| REDIS_URL | 服务自定义 | Redis 连接 |
| RUST_LOG | info | 日志级别 |
| SWAGGER_ENABLED | true | 是否挂 SwaggerUi |
| CONSUL_ADDR | http://localhost:8500 | Consul 地址 |
| JWT_PRIVATE_KEY_PATH | keys/private.pem | JWT 私钥路径（RS256 签发用） |
| JWT_PUBLIC_KEY_PATH | keys/public.pem | JWT 公钥路径（JWKS endpoint 暴露） |
| SERVICE_BOOTSTRAP_TOKENS | （空） | 各业务服务的逗号分隔 bootstrap token；未设置则 POST /tokens/service-jwt 拒绝所有调用方 |

## 端点域

| 端点 | 方法 | 说明 |
|------|------|------|
| /tokens/access | POST | 签发 access_token + refresh_token（OAuth 2.0 client_credentials / authorization_code） |
| /tokens/refresh | POST | 刷新 token（refresh_token 单次使用，防重放） |
| /tokens/revoke | POST | 撤销 token（写入 Redis 黑名单） |
| /tokens/resource | POST | 签发受限资源令牌（自定义 TTL 与 scope） |
| /tokens/user-jwt | POST | 签发用户身份 JWT |
| /tokens/service-jwt | POST | 签发 1h 服务间 JWT（Authorization: Bearer <SERVICE_BOOTSTRAP_TOKENS> 准入） |
| /tokens/.well-known/jwks.json | GET | JWKS 公钥（kid="default"，RS256） |
| /.well-known/jwks.json | GET | JWKS 别名 |
| /api-keys | POST / GET | API Key 创建 / 列表 |
| /admin/oauth-clients | POST / GET | OAuth 客户端创建（响应中一次性返回明文 client_secret） |
| /admin/oauth-clients/{client_id} | PUT / DELETE | 更新 / 删除 |
| /admin/oauth-clients/{client_id}/reset-secret | POST | 重置并返回新明文 client_secret |
| /health | GET | 健康检查 |
| /metrics | GET | Prometheus 指标 |
| /swagger-ui | GET | OpenAPI UI（仅 SWAGGER_ENABLED=true 时暴露） |

## 内部端点 /token/internal/*

| 端点 | 方法 | 说明 |
|------|------|------|
| /token/internal/migrations/state | GET | schema 状态查询（Phase 10 G3）|

- 返回 1 个 V001 migration 的 marker-table 探测结果
- 鉴权：service-jwt `aud=token-service`（Phase 10 G3 service-jwt 扩面）
- 详见 `docs/guides/migrations-state.md`

## 豁免

（无）

## 表命名约定

所有表必须以 `token_` 前缀（共享 `superpowers` MySQL 数据库）。详见 `contracts/db-conventions.md`，CI 网关：`node scripts/check-db-naming.js`。

## AI 检查时机

修改 services/token-service/ 下文件时自动检查

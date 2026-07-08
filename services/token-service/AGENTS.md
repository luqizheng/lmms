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

## 端点域

| 端点 | 方法 | 说明 |
|------|------|------|
| /token/issue | POST | 签发 token |
| /token/refresh | POST | 刷新 token |
| /token/revoke | POST | 撤销 token |
| /token/jwks | GET | JWKS 公钥 |
| /health | GET | 健康检查 |
| /metrics | GET | Prometheus 指标 |
| /swagger-ui | GET | OpenAPI UI（SWAGGER_ENABLED=true）|

## 内部端点 /token/internal/*

| 端点 | 方法 | 说明 |
|------|------|------|
| /token/internal/migrations/state | GET | schema 状态查询（Phase 10 G3）|

- 返回 1 个 V001 migration 的 marker-table 探测结果
- 当前无鉴权层（运维仅限内网访问，生产部署需走 service-jwt 扩面）
- 详见 `docs/guides/migrations-state.md`

## 豁免

（无）

## 表命名约定

所有表必须以 `token_` 前缀（共享 `superpowers` MySQL 数据库）。详见 `contracts/db-conventions.md`，CI 网关：`node scripts/check-db-naming.js`。

## AI 检查时机

修改 services/token-service/ 下文件时自动检查

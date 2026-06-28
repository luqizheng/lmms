# api-gateway 服务规范

## 必需组件

- [x] Dockerfile
- [x] AGENTS.md
- [x] README.md
- [x] tests/ 目录
- [x] /health 端点
- [x] /metrics 端点
- [ ] migrations/ 目录（豁免，见 ## 豁免）
- [ ] Swagger 配置（豁免，聚合视图留作后续阶段）

## 环境变量

| 变量 | 默认值 | 说明 |
|------|--------|------|
| SERVER_HOST | 0.0.0.0 | 监听主机 |
| SERVER_PORT | 3000 | 监听端口（对外网关固定 3000）|
| SERVER_ADDR | 0.0.0.0:3000 | 完整监听地址 |
| CONSUL_ADDR | http://localhost:8500 | Consul 地址 |
| RUST_LOG | info | 日志级别 |
| SWAGGER_ENABLED | false | 预留（聚合 UI 待 P2.1）|
| GW_REDIS__PREFIX | superpowers: | Redis key 前缀（blacklist + perm_epoch）；**必须与 iam REDIS_PREFIX 一致** |
| GW_EVENT_BUS_URL | amqp://guest:guest@localhost:5672/%2f | RabbitMQ 连接（perm_epoch 兜底 consumer，Phase 4.4）|
| GW_PERM_EPOCH_TTL_SEC | 900 | perm_epoch 兜底写入 TTL；**应与 iam `access_token_ttl_minutes*60` 对齐** |

## 令牌失效 perm_epoch（Phase 4.4）

- `auth_middleware` 在 blacklist 检查后比对 `token.iat < {prefix}perm_epoch:{sub}` → 401「permissions changed, re-authenticate」。key miss / Redis 错误 → fail-open 放行。
- 兜底 consumer：ephemeral 队列订阅 `iam.permission.changed`，用 set-if-greater（单调）补写 perm_epoch（用事件 `occurred_at` 作时间基准），关闭 iam 直写的 fail-open 窗口。

## 端点域

| 端点 | 方法 | 说明 |
|------|------|------|
| /health | GET | 健康检查 |
| /metrics | GET | Prometheus 指标 |
| /__admin/reload | POST | 热加载路由配置 |
| /api/v1/* | * | 业务路由代理（按 routes/*.router.toml 转发）|

## 豁免

- migrations/ 目录
- Swagger 配置（SWAGGER_ENABLED 开关）

## AI 检查时机

修改 services/api-gateway/ 下文件时自动检查

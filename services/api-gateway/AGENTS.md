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
| GW_JWKS_URL | jwks_url | http://127.0.0.1:4000/.well-known/jwks.json（代码默认；docker-compose 部署时覆盖为 http://token-service:8082/.well-known/jwks.json） |
| GW_ISSUER | issuer | superpowers-token-service |
| GW_ROUTES_DIR | routes_dir | /app/config/api-gateway/routes |

## 令牌失效 perm_epoch（Phase 4.4）

- `auth_middleware` 在 blacklist 检查后比对 `token.iat < {prefix}perm_epoch:{sub}` → 401「permissions changed, re-authenticate」。key miss / Redis 错误 → fail-open 放行。
- 兜底 consumer：ephemeral 队列订阅 `iam.permission.changed`，用 set-if-greater（单调）补写 perm_epoch（用事件 `occurred_at` 作时间基准），关闭 iam 直写的 fail-open 窗口。
- **行为变更（Phase 6-A T3）：** Redis SETEX 失败时 consumer 返回 `HandlerError::Transient`（旧版 swallow 返 Ok）。broker 会重投最多 3 次，之后消息进入 DLQ。瞬时 Redis 抖动会触发 bounded redelivery，不会静默丢失。

## 端点域

| 端点 | 方法 | 说明 |
|------|------|------|
| /health | GET | 健康检查 |
| /metrics | GET | Prometheus 指标 |
| /__admin/reload | POST | 热加载路由配置 |
| /api/v1/* | * | 业务路由代理（按 routes/*.router.toml 转发）|

## 业务服务注册（egress_map）

`GatewayConfig.egress_map` 决定 `route.service` 名 → 后端 host:port 的映射。

- 默认 closure（`src/config.rs:94`）全部 hardcode `port: 8080`。
- 当业务服务端口**非 8080** 时，必须在 `infrastructure/config/api-gateway/api-gateway.toml` 用 `[egress_map]` 子表显式 override。
- 现状：`file-service:8082`、`config-service:8081` 已在 override 列表中；其余五服务使用默认 8080。
- **Docker 部署必读**：`api-gateway` 服务的 volumes 必须 bind-mount 该 toml 到容器内 `/app/config/api-gateway.toml`（二进制从 `config/api-gateway` 加载），否则 override 不会生效，`router` 会打 ERROR `no egress configured for service 'xxx'`。
- 新增非 8080 服务时：写 `.router.toml` + 在 `api-gateway.toml` 加 override 行 + 在 `docker-compose-services.yml` 的 `api-gateway.volumes` 加 toml bind-mount + 跑 `cargo run -p api-gateway -- validate`。

## 豁免

- migrations/ 目录
- Swagger 配置（SWAGGER_ENABLED 开关）

## AI 检查时机

修改 services/api-gateway/ 下文件时自动检查

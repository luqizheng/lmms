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

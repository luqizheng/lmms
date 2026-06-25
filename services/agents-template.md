# <service-name> 服务规范

## 必需组件

- [ ] Dockerfile
- [ ] AGENTS.md
- [ ] README.md
- [ ] tests/ 目录
- [ ] /health 端点
- [ ] /metrics 端点
- [ ] migrations/ 目录
- [ ] Swagger 配置（SWAGGER_ENABLED 开关）

## 环境变量

| 变量 | 默认值 | 说明 |
|------|--------|------|
| SERVER_HOST | 0.0.0.0 | 监听主机 |
| SERVER_PORT | 8080 | 监听端口 |
| SERVER_ADDR | 0.0.0.0:8080 | 完整监听地址（优先级高于上两项）|
| DATABASE_URL | 服务自定义 | 数据库连接 |
| RUST_LOG | info | 日志级别 |
| SWAGGER_ENABLED | false | 是否挂 SwaggerUi |
| CONSUL_ADDR | 服务自定义 | Consul 地址 |

## 端点域

| 端点 | 方法 | 说明 |
|------|------|------|
| /health | GET | 健康检查 |
| /metrics | GET | Prometheus 指标 |
| /swagger-ui | GET | OpenAPI UI（SWAGGER_ENABLED=true 时启用）|

## 豁免

（无豁免则保留空段；豁免项写在此处，每行一项必需组件名）

## AI 检查时机

修改 services/<name>/ 下文件时自动检查

# config-service 服务规范

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
| DATABASE_URL | 服务自定义 | 数据库连接 |
| RUST_LOG | info | 日志级别 |
| SWAGGER_ENABLED | false | 是否挂 SwaggerUi |

## 端点域

| 端点 | 方法 | 说明 |
|------|------|------|
| /config/namespaces | GET / POST | 命名空间管理 |
| /config/items | GET / POST / PUT / DELETE | 配置项 CRUD |
| /config/watch | GET (SSE) | 配置变更订阅 |
| /health | GET | 健康检查 |
| /metrics | GET | Prometheus 指标 |
| /swagger-ui | GET | OpenAPI UI（SWAGGER_ENABLED=true）|

## 豁免

（无）

## AI 检查时机

修改 services/config-service/ 下文件时自动检查

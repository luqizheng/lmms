# file-service 服务规范

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
| STORAGE_PATH | ./data/files | 本地存储目录 |
| RUST_LOG | info | 日志级别 |
| SWAGGER_ENABLED | false | 是否挂 SwaggerUi |

## 端点域

| 端点 | 方法 | 说明 |
|------|------|------|
| /file/upload | POST | 文件上传（硬限 100MB，配置见 `src/config.rs`） |
| /file/list | GET | 文件列表查询（分页 + tenant 隔离） |
| /file/:id | GET | 下载文件内容 |
| /file/:id | DELETE | 删除文件 |
| /file/:id/info | GET | 文件元信息 |
| /file/presign | POST | 预签名上传 URL |
| /health | GET | 健康检查 |
| /metrics | GET | Prometheus 指标 |
| /swagger-ui | GET | OpenAPI UI（SWAGGER_ENABLED=true）|

## 内部端点 /file/internal/*

| 端点 | 方法 | 说明 |
|------|------|------|
| /file/internal/migrations/state | GET | schema 状态查询（Phase 10 G3）|

- 返回 2 个 V001-V002 migration 的 marker-table 探测结果
- 鉴权：service-jwt `aud=file-service`（Phase 10 G3 service-jwt 扩面）
- 详见 `docs/guides/migrations-state.md`

## 豁免

（无）

## 表命名约定

所有表必须以 `file_` 前缀（共享 `superpowers` MySQL 数据库）。详见 `contracts/db-conventions.md`，CI 网关：`node scripts/check-db-naming.js`。

## AI 检查时机

修改 services/file-service/ 下文件时自动检查

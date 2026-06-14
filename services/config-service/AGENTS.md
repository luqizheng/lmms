# config-service 开发规范

## 必须项

- [x] Dockerfile
- [x] 健康检查端点（/health）
- [x] 数据库迁移文件（migrations/）
- [x] config.yaml
- [x] AGENTS.md

## 环境变量

| 变量 | 默认值 | 说明 |
|------|--------|------|
| DATABASE_URL | sqlite::memory: | 数据库连接 |
| SERVER_ADDR | 0.0.0.0:8081 | 服务监听地址 |
| RUST_LOG | info | 日志级别 |

## 端点域

| 端点 | 方法 | 说明 |
|------|------|------|
| /health | GET | 健康检查 |
| /config/namespace | POST | 创建命名空间 |
| /config/namespace | GET | 查询命名空间列表 |
| /config/namespace/{id} | GET/PUT/DELETE | 命名空间 CRUD |
| /config/item | POST | 创建配置项 |
| /config/item | GET | 查询配置项列表 |
| /config/item/{id} | GET/PUT/DELETE | 配置项 CRUD |
| /config/watch | GET | 长轮询监听变更 |
| /config/sse | GET | SSE 推送 |
| /config/export/{namespace_id} | GET | 导出配置 JSON |
| /config/import/{namespace_id} | POST | 导入配置 JSON |

## 数据库方言

支持 MySQL 和 SQLite，通过 DATABASE_URL 自动检测。

# audit-service 开发规范

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
| SERVER_ADDR | 0.0.0.0:8080 | 服务监听地址 |
| RUST_LOG | info | 日志级别 |

## 端点域

| 端点 | 方法 | 说明 |
|------|------|------|
| /health | GET | 健康检查 |
| /audit/operation | POST | 创建操作日志 |
| /audit/operation | GET | 查询操作日志 |
| /audit/login | POST | 创建登录日志 |
| /audit/login | GET | 查询登录日志 |
| /audit/export/operation/csv | GET | 导出操作日志 CSV |

## 数据库方言

支持 MySQL 和 SQLite，通过 DATABASE_URL 自动检测。

# file-service 开发规范

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
| SERVER_ADDR | 0.0.0.0:8082 | 服务监听地址 |
| RUST_LOG | info | 日志级别 |
| STORAGE_PATH | ./data/files | 文件存储路径 |

## 端点域

| 端点 | 方法 | 说明 |
|------|------|------|
| /health | GET | 健康检查 |
| /file/upload | POST | 上传文件 |
| /file/list | GET | 查询文件列表 |
| /file/{id} | GET | 下载文件 |
| /file/{id} | DELETE | 删除文件 |
| /file/{id}/info | GET | 获取文件元数据 |
| /file/presign | POST | 创建预签名 URL |

## 存储后端

默认使用本地文件系统（LocalStorage），支持 ObjectStore trait 扩展 S3 等。

## 数据库方言

支持 MySQL 和 SQLite，通过 DATABASE_URL 自动检测。
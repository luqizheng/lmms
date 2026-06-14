# file-service

文件服务，提供文件上传、下载、预签名 URL 功能。

## 功能

- 文件上传：单文件上传，支持元数据
- 文件下载：直链下载，支持过期检查
- 预签名 URL：临时访问 URL，TTL 可配
- 文件管理：查询、删除、元数据获取
- 临时文件：支持过期自动清理

## 快速开始

```bash
# SQLite 测试
DATABASE_URL=sqlite::memory: STORAGE_PATH=./data/files cargo run

# MySQL
DATABASE_URL=mysql://user:pwd@localhost/file cargo run
```

## API

```bash
# 上传文件
curl -X POST http://localhost:8082/file/upload \
  -H "Content-Type: application/json" \
  -d '{"filename":"test.txt","content_type":"text/plain"}' \
  --data-binary @test.txt

# 查询文件列表
curl http://localhost:8082/file/list

# 下载文件
curl http://localhost:8082/file/{id}

# 创建预签名 URL
curl -X POST http://localhost:8082/file/presign \
  -H "Content-Type: application/json" \
  -d '{"file_id":"file-id","operation":"get","expires_in_seconds":3600}'

# 删除文件
curl -X DELETE http://localhost:8082/file/{id}
```

## 环境变量

- DATABASE_URL: 数据库连接
- SERVER_ADDR: 监听地址（默认 0.0.0.0:8082）
- RUST_LOG: 日志级别（默认 info）
- STORAGE_PATH: 文件存储路径（默认 ./data/files）

## 存储后端

默认使用本地文件系统，可通过 ObjectStore trait 扩展：
- LocalStorage: 本地文件系统
- S3Storage: AWS S3（待实现）
- RustfsStorage: rustfs（待实现）

## 数据模型

### FileInfo
```json
{
  "id": "uuid",
  "filename": "string",
  "content_type": "string (optional)",
  "size": "number",
  "storage_path": "string",
  "tenant_id": "string (optional)",
  "user_id": "string (optional)",
  "is_temporary": "boolean",
  "expires_at": "datetime (optional)"
}
```

### PresignUrlResponse
```json
{
  "url": "string",
  "expires_at": "datetime",
  "operation": "string (get/put)"
}
```
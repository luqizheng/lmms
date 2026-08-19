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
# MySQL（仅支持 MySQL，已移除 SQLite）
DATABASE_URL=mysql://user:pwd@localhost/superpowers STORAGE_PATH=./data/files cargo run
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
- FILE_STORAGE_BACKEND: `local` / `rustfs` / `s3`，决定 storage driver（默认 `local`）
- STORAGE_PATH: 本地存储路径（默认 ./data/files；非 local 后端忽略）
- S3_ENDPOINT: S3 endpoint（rustfs 必填，如 http://rustfs:9000；AWS / Aliyun 可不填自动推断）
- S3_BUCKET: 桶名（默认 `coder-files`）
- S3_REGION: 区域（默认 `us-east-1`）
- S3_ACCESS_KEY / S3_SECRET_KEY: 凭据（rustfs / s3 必填）
- S3_PATH_STYLE: true/false（rustfs 自动 true；AWS / Aliyun 不需要）
- S3_AUTO_CREATE_BUCKET: true/false（默认 true；启动时尝试创建 bucket，失败仅 warn）

## 存储后端

通过 `ObjectStore` trait 抽象，运行时按 `FILE_STORAGE_BACKEND` env 选择 — 同一镜像支持多 driver：

| backend | 用途 | 典型场景 |
|---------|------|----------|
| `local`  | 本地文件系统（dev 默认）| 单机开发 / 单机测试 |
| `rustfs` | RustFS（S3 协议 + path-style）| **私有化部署推荐**（与本仓库"多客户用同一镜像"目标一致）|
| `s3`     | AWS S3 / Aliyun OSS / 腾讯 COS | 公有云部署 |

实现：
- `src/storage/local.rs` — 本地文件系统
- `src/storage/s3.rs` — 统一 S3 协议 driver，覆盖 RustFS / MinIO / AWS S3 / Aliyun OSS；
  通过 `S3Config`（bucket/region/endpoint/path_style/access_key/secret_key）区分 driver，**无代码分支差异**

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
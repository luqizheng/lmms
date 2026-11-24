# file-service

LMMS 内部文件对象服务，提供文件上传、下载、元数据查询、删除和对象存储预签名 URL。

> **内部服务：** 所有 `/file/*` 端点只接受 `aud=file-service` 的 service-jwt。用户 JWT 不可直接调用，api-gateway 不暴露用户文件路由。用户面权限由业务服务校验后再调用本服务。

## 能力边界

当前提供扁平文件对象管理，不是 POSIX 文件系统或网盘：不提供目录树、移动/复制、版本历史或回收站。文件字节由 LocalStorage 或 S3-compatible ObjectStore 管理，数据库保存元数据。

- 单文件 multipart 上传（默认最大 100 MiB）
- 文件列表、元数据、下载和删除
- S3-compatible GET/PUT 预签名 URL（TTL 1..604800 秒）；LocalStorage 不支持原生 presign，返回 501
- 临时文件过期清理：每 60 秒扫描，单轮最多 100 个；删除失败保留记录并重试
- 上传元数据落库失败时尝试补偿删除已写入对象

## 快速开始

```bash
DATABASE_URL=mysql://user:pwd@localhost/superpowers \
FILE_STORAGE_BACKEND=local STORAGE_PATH=./data/files cargo run
```

## API（内部调用）

所有请求均需 `Authorization: Bearer <service-jwt>`，且 JWT audience 必须是 `file-service`。

### multipart 上传

```bash
curl -X POST http://localhost:8082/file/upload \
  -H "Authorization: Bearer $FILE_SERVICE_JWT" \
  -F 'metadata={"filename":"test.txt","content_type":"text/plain","tenant_id":1}' \
  -F 'file=@test.txt'
```

### 查询、下载、预签名、删除

```bash
curl -H "Authorization: Bearer $FILE_SERVICE_JWT" \
  "http://localhost:8082/file/list?page=1&size=20"

curl -H "Authorization: Bearer $FILE_SERVICE_JWT" \
  "http://localhost:8082/file/{id}/info"

curl -X POST http://localhost:8082/file/presign \
  -H "Authorization: Bearer $FILE_SERVICE_JWT" \
  -H "Content-Type: application/json" \
  -d '{"file_id":"file-id","operation":"get","expires_in_seconds":300}'

curl -X DELETE http://localhost:8082/file/{id} \
  -H "Authorization: Bearer $FILE_SERVICE_JWT"
```

## 存储后端

通过 `ObjectStore` trait 抽象，运行时按 `FILE_STORAGE_BACKEND` 选择 driver：

| backend | 用途 | 典型场景 |
|---|---|---|
| `local` | 本地文件系统 | 开发、单机测试；多副本生产不建议 |
| `rustfs` | S3 协议 + path-style | 私有化部署 |
| `s3` | AWS S3 / Aliyun OSS / 腾讯 COS 等兼容服务 | 公有云或托管对象存储 |

环境变量：
- `DATABASE_URL`：必填数据库连接。
- `SERVER_ADDR`：监听地址，默认 `0.0.0.0:8082`。
- `FILE_STORAGE_BACKEND`：`local` / `rustfs` / `s3`，默认 `local`。
- `STORAGE_PATH`：本地存储根目录，默认 `./data/files`。
- `S3_ENDPOINT`、`S3_BUCKET`、`S3_REGION`、`S3_ACCESS_KEY`、`S3_SECRET_KEY`、`S3_PATH_STYLE`、`S3_AUTO_CREATE_BUCKET`：S3-compatible 后端配置。

## 一致性与运维

- 数据库元数据和对象存储内容必须一起备份、恢复和校验。
- 删除顺序为先删除对象、再删除元数据；元数据删除失败时可重试，对象删除必须保持幂等。
- 上传先写对象再写元数据；元数据写入失败会尝试补偿删除对象。补偿失败会记录 error 日志，需纳入运维告警/孤儿对象巡检。
- 切换存储后端必须迁移对象并校验数量、大小和 hash，不能只改环境变量。
- 详细规则见 `docs-site/guides/file-storage-usage.md`。

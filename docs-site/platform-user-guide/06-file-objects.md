# 06. 文件对象服务

## 6.1 能力边界

LMMS file-service 是文件对象管理服务，不是通用文件系统或网盘。

当前具备：上传、列表/分页、元数据查询、下载、删除、S3-compatible 预签名 GET/PUT、临时文件 TTL 和后台清理。

当前未提供通用目录树、文件夹、移动、复制、重命名、版本历史、回收站、文件锁、全文检索、预览和断点续传的完整契约。AI-PM 不应仅凭文件列表 API 设计出假定这些能力已经存在的 UI。

## 6.2 安全边界

- file-service 是内部服务；所有 `/file/*` 路径要求 `aud=file-service` service-jwt。
- 用户 JWT 不能直接调用 file-service；Gateway 不应将其当成用户公开 API。
- 用户面文件操作由业务服务提供接口。业务服务先验证用户、租户、业务对象和附件权限，再以 service-jwt 调用 file-service。
- `tenant_id` 由 file-service 从已验证 service-jwt 的 `tid` claim 取得；Rust SDK 的 `UploadFileMeta` 不包含 `tenant_id`。真实用户归属 `user_id` 由可信调用方在完成用户鉴权后传递。不要让客户端自行指定 tenant_id。
- 文件 ID 不是访问授权凭证。每次下载/删除前，业务服务都必须验证该文件与业务对象的关联。

## 6.3 上传流程

```text
Browser -> Business API (user JWT)
Business API: authorize user / tenant / business object
Business API -> file-service (service-jwt, aud=file-service)
file-service: store bytes + metadata
Business API: persist file_id relation
```

上传采用 multipart/form-data：

- `metadata`：JSON 字符串，描述 filename、content_type、user_id、临时文件属性等。
- `file`：原始文件字节。
- 当前文档记录默认上限 100 MiB；部署配置 `max_file_size` 是最终依据，客户端应读取/配置匹配限制。

示意（由后端发起，需先获取 service-jwt）：

```bash
curl -X POST "$FILE_SERVICE_INTERNAL/file/upload" \
  -H "Authorization: Bearer $FILE_SERVICE_JWT" \
  -F 'metadata={"filename":"design.pdf","content_type":"application/pdf","user_id":"user-123"};type=application/json' \
  -F 'file=@design.pdf'
```

实际部署若通过内部 DNS 调用，`FILE_SERVICE_INTERNAL` 是内部地址，不能提供给浏览器。

## 6.4 下载与预签名 URL

推荐下载流程：

1. 浏览器向业务服务请求下载某个业务附件。
2. 业务服务校验用户对业务对象的读取权限。
3. 业务服务以 service-jwt 调用 file-service 创建 GET presign。
4. S3-compatible 后端返回短时效预签名 URL，浏览器按该 URL 下载。
5. LocalStorage 不支持原生 presign，会返回 501；此时业务服务需调用内部下载接口并代理二进制响应。

预签名 URL 本身是 bearer capability：任何持有者在有效期内可能使用它。使用短 TTL，不要写入日志、分析事件或长期数据库字段。

## 6.5 删除和临时文件

- 删除前先解除业务记录与文件 ID 的关联，处理删除失败重试。
- 上传成功但业务事务失败时，需要定义补偿/孤儿文件清理策略。
- 临时文件默认 TTL 为 24 小时；后台按周期扫描并清理过期对象，删除失败会保留元数据以便重试。
- 不要把临时文件 TTL 当成业务归档或法定保留策略。

## 6.6 存储后端

| 后端 | 主要配置 | 使用建议 |
|---|---|---|
| local | `FILE_STORAGE_BACKEND=local`、`STORAGE_PATH` | 开发/单机测试；多副本避免各自本地目录 |
| rustfs | `FILE_STORAGE_BACKEND=rustfs`、S3 endpoint/bucket/credentials | 私有化部署 |
| s3 | S3 bucket/region/credentials，可选 endpoint | 云对象存储或兼容服务 |

数据库元数据和对象字节是两个持久化面。备份/恢复必须同时覆盖两者，并抽样核验文件 ID、对象、大小和 hash。

## 6.7 文件管理 UI 的正确抽象

可实现的 UI：
- 文件列表（文件名、类型、大小、上传者、创建时间、租户/业务归属）。
- 文件详情（元数据、引用业务对象）。
- 上传进度（由业务服务上传接口支持）。
- 下载按钮（先请求业务服务授权，再跳转预签名 URL 或代理下载）。
- 删除（由业务服务校验权限并执行）。
- 临时文件标识和过期提示。

需要新增 LMMS API/数据模型后才能实现：
- 文件夹/目录树、移动、复制、重命名。
- 版本历史、回收站、恢复。
- 分块/断点续传、预览、病毒扫描、全文检索。
- 跨业务系统的统一附件引用和共享授权。

## 6.8 依据

完整存储细节见 [文件服务与文件存储使用规范](../guides/file-storage-usage.md)，接口见 `services/file-service/openapi.yaml`。

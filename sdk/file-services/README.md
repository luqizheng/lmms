# file-services

LMMS file-service 的 TypeScript 服务端 SDK：multipart 上传、二进制下载、文件管理与预签名 URL。使用原生 `fetch`、`FormData` 和 `Blob`，运行环境为 Node.js 18+。

> **安全边界：仅供受信任的后端服务使用。禁止在浏览器、桌面客户端或不可信客户端中使用。** file-service 是纯内部服务，所有 `/file/*` 端点只接受 `aud=file-service` 的 service-jwt。不得传入用户 access token，也不得把 service-jwt 暴露给前端。用户授权由业务服务完成。

## 快速上手

```ts
import { createFileServices } from "@digua/file-services";

const file = createFileServices({
  // 内部网络中的 file-service 地址，不是 api-gateway 公共地址
  baseUrl: process.env.FILE_SERVICE_URL ?? "http://file-service:8082",
  // 必须是 aud=file-service 的 service-jwt
  getToken: () => serviceJwtCache.get("file-service"),
});

// 上传：multipart metadata JSON + file 字节；服务端默认上限 100 MiB
const info = await file.files.upload(
  {
    filename: "design.pdf",
    content_type: "application/pdf",
    is_temporary: true,
    expires_in_seconds: 3600,
  },
  pdfBytes,
);

// 业务服务必须先验证当前用户是否有权访问该业务附件，再创建下载授权
const signed = await file.presign.create({
  file_id: info.id,
  operation: "get",
  expires_in_seconds: 300,
});
// 将 signed.url 返回给已通过业务授权的用户；不要持久化或记录完整签名 URL
```

## API

| 模块 | 方法 | 内部端点 |
|---|---|---|
| `files` | `upload` | `POST /file/upload`（multipart） |
| | `page` | `GET /file/list` |
| | `info` | `GET /file/{id}/info` |
| | `download` | `GET /file/{id}` |
| | `remove` | `DELETE /file/{id}` |
| `presign` | `create` | `POST /file/presign` |

预签名 operation 仅允许：
- `get`：读取已存在的对象。
- `put`：向该文件对象的存储键写入/覆盖内容。

TTL 必须在 1 到 604800 秒（7 天）之间。业务下载通常应使用更短 TTL。\n\n原生预签名 URL 仅适用于 S3-compatible 后端（RustFS / MinIO / AWS S3 / OSS 等）。LocalStorage 不支持原生预签名 URL，调用会返回 501；本地开发应由可信业务服务使用 service-jwt 调用内部下载接口并代理响应。

## 文件归属与授权

- file-service 根据 service-jwt 中的租户范围执行租户隔离。
- 上传的 `tenant_id` 由可信调用上下文决定；业务服务必须先认证用户并验证业务归属。
- 查询、下载、删除和创建预签名 URL 前，业务服务必须验证文件 ID 与业务对象之间的关联关系。
- `storage_path` 是内部对象键，不能作为用户路径或公开 URL。
- 用户下载应使用经过业务授权后创建的短期 `get` 预签名 URL。

## 临时文件生命周期

- `is_temporary=true` 时，未指定 TTL 默认 24 小时。
- 临时文件 TTL 必须为正数。
- file-service 后台每 60 秒扫描一次过期临时文件，每轮最多处理 100 个。
- 对象删除失败会保留元数据并在后续周期重试。
- 非临时文件不因 `expires_in_seconds` 被自动过期。

## 错误模型

SDK 抛出 `FileServicesError`（`status` / `message`）。上传超限返回 413；非法 presign operation 在 JSON 反序列化阶段返回 4xx；TTL 越界返回 400。

## 开发

```bash
pnpm --filter @digua/file-services test
pnpm --filter @digua/file-services typecheck
pnpm --filter @digua/file-services build
```

发布走仓库统一管线（见根目录 `AGENTS.md) 的 npm 包发布规则）。

# file-services

lmms file-service SDK：**multipart 上传、二进制下载、文件管理、预签名 URL** 的 TypeScript 客户端。零运行时依赖（原生 `fetch` + `FormData`/`Blob`），浏览器与 Node ≥18 通用，面向 api-gateway 的 `/v1/file` 路径。

与 `iam-services` 同款模式（SDK 位置例外见 `sdk/README.md`）；安装与 `.npmrc` 配置见该包 README。

## 快速上手

```ts
import { createFileServices } from "@digua/file-services";

const file = createFileServices({
  baseUrl: "http://127.0.0.1:3000",
  getToken: () => localStorage.getItem("access_token") ?? undefined,
});

// 上传（multipart：metadata JSON 串 + file 字段；上限 100 MiB）
const info = await file.files.upload(
  { filename: "报告.pdf", content_type: "application/pdf", tenant_id: 1 },
  fileInput.files[0] // Blob | Uint8Array | ArrayBuffer
);

// 下载（返回字节 + 解析出的文件名）
const dl = await file.files.download(info.id);
new Blob([dl.bytes], { type: dl.contentType }); // filename: dl.filename

// 管理
const page = await file.files.page({ user_id: "usr-1", page: 1, size: 20 });
await file.presign.create({ file_id: info.id, operation: "download", expires_in_seconds: 300 });
```

## API 一览

| 模块 | 方法 | 端点 |
|---|---|---|
| `files` | `upload` | `POST /v1/file/upload`（multipart） |
| | `page` | `GET /v1/file/list`（分页） |
| | `info` / `download` / `remove` | `GET /v1/file/{id}/info`、`GET /v1/file/{id}`、`DELETE /v1/file/{id}` |
| `presign` | `create` | `POST /v1/file/presign` |

## 错误模型

统一抛 `FileServicesError`（`status` / `message`）。file 路由处理器错误体是**纯文本**（如 `"file not found"`、`"file too large"`、`"presign expired"`）。

## 陷阱

1. 上传上限默认 **100 MiB**，超出 413。
2. `FileInfo.url` 是服务端注入的网关相对路径 `/v1/file/{id}`（非完整 URL）。
3. 下载响应的 `Content-Type` 为上游存储值（回退 `application/octet-stream`）；文件名从 `Content-Disposition` 解析（RFC 5987 `filename*` 优先）。
4. 全部端点需要鉴权；`/v1/file/health` 是唯一公开路由。`/v1/file/internal/**` 需服务级 JWT，不在本 SDK 范围。

## 开发

```bash
pnpm --filter @digua/file-services test / typecheck / build
```

发布走仓库统一管线（见 `AGENTS.md`「npm 包发布规则」）。

# @digua/http

框架无关的 HTTP 客户端。

- **不绑定 UI 框架**：token 存取、刷新、错误文案翻译都由消费方注入，
  Vue 壳与 project-manager 共用同一份实现。
- **单飞刷新**：token 过期时并发请求只触发一次刷新，随后重放原请求。
- **统一错误**：后端业务错误转成 `HttpError`，带 `code` / `status` / `data`。

```ts
import { createHttp } from "@digua/http";

export const http = createHttp({
  baseURL: "/api",
  getToken: () => tokenStore.accessToken,
  refreshToken: () => tokenStore.refresh(),
  onUnauthorized: () => router.push("/login"),
  translateError: code => translateError(code)
});
```

# prue-services

lmms prue_backend SDK：**动态路由（get-async-routes）与菜单配置版本管理**的 TypeScript 客户端。零运行时依赖（原生 `fetch`），浏览器与 Node ≥18 通用，面向 api-gateway 的 `/v1/pure` 路径。ai-pm 等 PRISM 壳的菜单/动态路由调用即来源于此服务。

与 `iam-services` 同款模式（SDK 位置例外见 `sdk/README.md`）；安装与 `.npmrc` 配置见该包 README。

## 快速上手

```ts
import { createPrueServices } from "@digua/prue-services";

const prue = createPrueServices({
  baseUrl: "http://127.0.0.1:3000",
  getToken: () => localStorage.getItem("access_token") ?? undefined,
});

// 动态路由（路由守卫加载菜单）
const routes = await prue.routes.getAsync(); // RouteNode[]，无激活配置时为 []

// 菜单配置版本管理（ETag 乐观锁）
const { items } = await prue.configs.list();
const draft = await prue.configs.create({ name: "v2", routes_json: routes });
const detail = await prue.configs.get(draft.id);
try {
  await prue.configs.update(draft.id, { routes_json: newRoutes, if_match_etag: detail.routes_etag });
  await prue.configs.publish(draft.id);
} catch (e) {
  if (e instanceof PrueServicesError && e.status === 409) {
    const current = (e.body as { data: { current_etag: string } }).data.current_etag; // 重新拉取后重试
  }
}
```

## API 一览

| 模块 | 方法 | 端点 |
|---|---|---|
| `routes` | `getAsync` | `GET /v1/pure/get-async-routes` → `RouteNode[]` |
| `configs` | `list` / `create` / `get` / `update` / `remove` / `clone` / `publish` | `/v1/pure/menu-configs*` |

## 错误模型

统一抛 `PrueServicesError`。本服务**全响应包 `{code,message,data}` 信封**（openapi 未建模该包装，以处理器实际输出为准）：SDK 解包成功响应；信封 `code!==0` 或 HTTP 错误都归一为该错误（`code` 为数字、镜像 HTTP 状态）。

## 陷阱

1. **ETag 乐观锁**：更新草稿必须带 `if_match_etag`（读取时的 `routes_etag`）；缺失 400、冲突 409（`error.body.data.current_etag` 为服务端当前值）。
2. `routes_json` 必须是**数组**（服务端校验），元素仅弱校验（`path` 必填且唯一）。
3. 已发布版本不可删除（409）、不可更新（状态冲突）。
4. **上游防护现状**：本服务自身不校验 JWT，鉴权完全依赖网关（合法低权限用户 token 可直达服务端接口）——属上游已知问题，已记录待修。
5. **上游缺陷已修**：`/v1/pure/health` 曾 404（网关 rewrite 指错路径），已随网关配置修正。

## 开发

```bash
pnpm --filter @digua/prue-services test / typecheck / build
```

发布走仓库统一管线（见 `AGENTS.md`「npm 包发布规则」）。

# iam-services

lmms IAM SDK：**登录 / 用户管理 / token 管理**的 TypeScript 客户端。零运行时依赖（原生 `fetch`），浏览器与 Node ≥18 通用，面向 api-gateway 的 `/v1` 公开路径。

> 位置说明：npm TS 包按仓库约定应放顶层 `packages/`；本包经用户裁决（2026-10-31）作为例外放在 `sdk/iam-services`，并已接入公开发布管线（`packages/cli/lib/sdk-publish.js` 清单）。

## 安装

公开发布到 npmjs.org（统一使用 `@digua/` scope）：

```bash
pnpm add @digua/iam-services
```

## 快速上手

```ts
import { createIamServices } from "@digua/iam-services";

const iam = createIamServices({
  // 默认 http://127.0.0.1:3000（本机 api-gateway）；浏览器同域反代可传相对路径如 "/api-gw"
  baseUrl: "http://127.0.0.1:3000",
  // 受保护端点（登出/用户管理/API Key…）自动附加 Bearer；token 由你的应用层管理
  getToken: () => localStorage.getItem("access_token") ?? undefined,
});

// 登录（公开端点）
const res = await iam.auth.login({ username: "admin", password: "••••••" });
// res: { access_token, refresh_token, token_type, expires_in: 900, user: { id, username, nickname, roles[], permissions[] } }

// 用户管理（需鉴权）
const simple = await iam.users.pageSimple({ keyword: "张", page: 1, size: 20 });
await iam.users.assignRoles("usr-1", ["role-id-1"]);

// token 管理（OAuth 机器身份链路）
const tok = await iam.tokens.issueAccess({ grant_type: "client_credentials", client_id: "app", client_secret: "…" });
await iam.tokens.revoke({ token_jti: "…", expires_in: 300 });
```

## API 一览

| 模块 | 方法 | 端点 | 鉴权 |
|---|---|---|---|
| `auth` | `login` | `POST /v1/iam/auth/login` | 公开 |
| | `refresh` | `POST /v1/iam/auth/refresh` | 公开 |
| | `logout` | `POST /v1/iam/auth/logout` | Bearer |
| | `setupStatus` | `GET /v1/iam/auth/setup-status` | 公开 |
| | `register` | `POST /v1/iam/auth/register`（仅首个用户） | 公开 |
| | `changePassword` | `POST /v1/iam/auth/password` | Bearer |
| | `listSessions` | `GET /v1/iam/auth/sessions` | Bearer |
| | `revokeSession` | `DELETE /v1/iam/auth/sessions/{id}` | Bearer |
| `users` | `page` / `pageSimple` / `get` / `create` / `update` / `remove` / `assignRoles` | `/v1/iam/users…` | Bearer |
| `roles` | `list` / `get` / `create` / `update` / `remove` | `/v1/iam/roles…` | Bearer |
| `tokens` | `issueAccess` | `POST /v1/tokens/access` | 公开 |
| | `refresh`（OAuth 链路） | `POST /v1/tokens/refresh` | 公开 |
| | `revoke` | `POST /v1/tokens/revoke` | 公开 |
| | `jwks` | `GET /v1/tokens/.well-known/jwks.json` | 公开 |
| `tokens.apiKeys` | `list` / `create` | `GET/POST /v1/api-keys` | Bearer |

## 错误模型

所有失败抛 `IamServicesError`（`name === "IamServicesError"`）：

- `status` — HTTP 状态码
- `message` — 纯文本错误体（iam-service 实际行为，如 `"invalid credentials"`）或结构化错误的 `message`
- `code` — 错误码（如 `"IAM_AUTH_1001"`），仅当服务端返回 `{code,message}` 结构体时存在
- `body` — 结构化错误体的原始 JSON（如有）

```ts
try {
  await iam.auth.login({ username: "admin", password: "wrong" });
} catch (e) {
  if (e instanceof IamServicesError && e.status === 401) {
    // 提示用户名或密码错误
  }
}
```

## 行为与陷阱（重要）

1. **SDK 不做自动刷新**。单飞刷新（single-flight）留在消费方的 HTTP 层（如 `@digua/http` 的 `refreshToken` 钩子）；`auth.refresh()` 显式暴露，由应用层决定调用时机。
2. **iam 登录链路的 refresh token 不轮换**：`auth.refresh()` 响应中的 `refresh_token` 与传入值相同，**不要**用它覆盖已存储的 refresh token。`tokens.refresh()`（OAuth 链路）则相反——单次使用轮换 + 重放检测，新 token 必须采用。
3. **access token 默认 900 秒**、iam 链路 refresh token 30 天（`expires_in` 以响应为准）。
4. **会话端点用 `/v1/iam/auth/sessions`**。网关上 `/v1/iam/sessions` 路由指向不存在的下游路径，勿用。
5. **`apiKeys.create` 的 secret 仅返回一次**，落库前务必保存。
6. 登出会撤销该用户**全部** refresh token 并拉黑当前 access token 的 jti（Redis 黑名单）。

## 开发

```bash
pnpm install            # 仓库根
pnpm --filter @digua/iam-services test      # vitest
pnpm --filter @digua/iam-services typecheck
pnpm --filter @digua/iam-services build     # 产物 dist/
```

发布走仓库统一管线（git tag 为版本单一事实源）：`pnpm run packages:check`（守卫干跑）/ `pnpm run packages:publish --version <x.y.z>`（私有 Verdaccio）。

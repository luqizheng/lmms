# notification-services

lmms notification-service SDK：**公告管理与未读数**的 TypeScript 客户端。零运行时依赖（原生 `fetch`），浏览器与 Node ≥18 通用，面向 api-gateway 的 `/v1/notification` 路径。**不含 WS 推送**（见陷阱）。

与 `iam-services` 同款模式（SDK 位置例外见 `sdk/README.md`）；安装与 `.npmrc` 配置见该包 README。

## 快速上手

```ts
import { createNotificationServices } from "notification-services";

const ntf = createNotificationServices({
  baseUrl: "http://127.0.0.1:3000",
  getToken: () => localStorage.getItem("access_token") ?? undefined,
});

// 当前用户视角
const list = await ntf.announcements.listMine({ pinned: true }); // 裸数组
const unread = await ntf.unread.count();
await ntf.announcements.markRead(1);

// 管理员视角（403 = 无权限）
const { id } = await ntf.announcements.create({
  title: "停机公告", body: "...", publish_at: "2026-11-01T00:00:00Z",
  pinned: true, popover: false,
  recipients: [{ scope_type: "role", scope_id: "admin" }], // user | role | dept
});
await ntf.announcements.publish(id);
```

## API 一览

| 模块 | 方法 | 端点 |
|---|---|---|
| `announcements` | `listMine` | `GET /v1/notification/announcements`（裸数组，无分页） |
| | `create`（管理员） | `POST /v1/notification/announcements` → 201 `{id}` |
| | `get` / `publish` / `revoke` / `markRead` | `/v1/notification/announcements/{id}...` |
| `unread` | `count` | `GET /v1/notification/unread-count` → `{count}` |

## 错误模型

统一抛 `NotificationServicesError`（`status` / `code` / `message`）。notification 的错误体是 `{error: "..."}`（上游错误为 `{error:"upstream", code, message}`）——SDK 归一后 `message` 取可读文本、`code` 取上游错误码（如有）。

## 陷阱

1. **`pinned`/`popover` 不对称**：创建时传 `boolean`，响应里是 `0/1` 整数（后端如此，SDK 照实镜像）。
2. **WS 推送不在本 SDK**：`/v1/notification/ws` 是 WebSocket（`ws=true` 网关流式透传），且上游以可伪造的 `x-user-id` 头识别用户——等上游修复身份机制后再纳入。
3. `create`/`publish`/`revoke` 需管理员权限，否则 403。
4. **上游缺陷已修**：`/v1/notification/health` 曾 404（网关 rewrite 指错路径），已随网关配置修正。

## 开发

```bash
pnpm --filter notification-services test / typecheck / build
```

发布走仓库统一管线（见 `AGENTS.md`「npm 包发布规则」）。

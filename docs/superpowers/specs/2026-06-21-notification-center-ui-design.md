# notification-service 前端 UI 设计

> 日期：2026-06-21
> 范围：纯前端（`frontend/src/**`）+ 少量后端配合（一个枚举端点 + WS 鉴权 header）
> 状态：待用户审阅

---

## 1. 目标

为 `notification-service` 提供完整的前端 UI，覆盖：

1. **管理端**：创建 / 发布 / 撤销 / 列表公告，含收件人选择（部门 / 角色 / 用户三栏）
2. **用户端**：收件箱列表、详情、标记已读、头部未读徽标
3. **WebSocket**：登录后 Layout 全局连接，断线指数退避，收到推送刷新徽标与弹层

完成标志：API + UI e2e 全部通过，UI 中 admin 与 user 双身份端到端验证 WS 推送链路。

---

## 2. 路由与页面

新增三个路由（`frontend/src/router/modules/notification.ts`）：

| 路径 | 名称 | 组件 | 鉴权 |
|---|---|---|---|
| `/notification/inbox` | NotificationInbox | `views/notification/inbox/index.vue` | 任意登录用户 |
| `/notification/admin` | NotificationAdmin | `views/notification/admin/index.vue` | admin only |
| `/notification/detail/:id` | NotificationDetail | `views/notification/detail/index.vue` | 任意登录用户 |

- 顶部菜单新增「通知中心」分组，挂在「系统管理」之后：
  - `收件箱` → `/notification/inbox`
  - `公告管理` → `/notification/admin`（仅 admin 可见，通过 `meta.roles` 限制）
- `views/welcome/` 头部不修改

---

## 3. 数据流

### 3.1 状态层（Pinia）

新增 `frontend/src/store/modules/notification.ts`：

```ts
export const useNotificationStore = defineStore("notification", () => {
  const unreadCount = ref(0);
  const popoverQueue = ref<NotificationItem[]>([]);
  const lastFetchedAt = ref(0);

  async function refreshUnread() { /* GET /unread-count */ }
  function pushFromSocket(item: NotificationItem) {
    popoverQueue.value.push(item);
    unreadCount.value += 1;
  }
  function markRead(id: number) {
    unreadCount.value = Math.max(0, unreadCount.value - 1);
  }
  return { unreadCount, popoverQueue, refreshUnread, pushFromSocket, markRead };
});
```

### 3.2 WS 单例

`frontend/src/utils/notification-socket.ts`：

```ts
let ws: WebSocket | null = null;
let retry = 0;
const handlers = new Map<string, Set<(p: any) => void>>();

export function connect() {
  const proto = location.protocol === "https:" ? "wss" : "ws";
  const url = `${proto}://${location.host}/api/v1/notification/ws`;
  // 浏览器自动携带 Authorization header；gateway 校验后注入 x-user-id
  ws = new WebSocket(url);
  ws.onmessage = e => {
    const msg = JSON.parse(e.data);
    handlers.get(msg.type)?.forEach(h => h(msg));
  };
  ws.onclose = () => scheduleReconnect();
}

export function on(type: string, h: (p: any) => void) { /* register */ }
export function disconnect() { /* close + clear retry */ }
```

生命周期：

- 监听 `userStore.token`：token 出现 → `connect`，token 清空 → `disconnect`
- 断线退避：1s → 2s → 4s → 8s → 16s → 30s（封顶）
- 收到 `{type:"announcement"}` → 调 `notificationStore.pushFromSocket()`
- 收到 `{type:"unread_count", count:N}` → 直接覆盖 `unreadCount`

### 3.3 收件人三栏（管理端）

**约束：不得修改 notification-service 后端源码**（项目硬规则）。

收件人数据来源由前端直接调 iam-service 网关端点：

- `GET /v1/iam/dept/list` → `ScopeItem[]`
- `GET /v1/iam/roles` → `ScopeItem[]`
- `GET /v1/iam/users` → `ScopeItem[]`

封装：`utils/iam-scopes.ts` 提供 `loadScopes(): Promise<ScopesResponse>`，内部并发 `Promise.all` 拉三个端点，结果本地缓存 5 分钟（in-memory + sessionStorage）。失败时降级为空数组（提示「数据加载失败」）。

- 前端三栏 el-tabs，每栏 search + el-select multiple
- 提交时拼装 `recipients: [{scope_type, scope_id}, ...]`

---

## 4. WS 鉴权

WS 连接在浏览器侧发起 WebSocket 升级时，浏览器会携带 `Authorization` header。api-gateway 收到升级请求后：

1. 校验 JWT（沿用 `api-gateway/jwt.rs`）
2. 注入 `x-user-id` / `x-user-roles` / `x-user-dept` headers
3. 透传到 `notification-service`

`notification-service/src/ws.rs` 当前仅读 `x-user-id`，已能工作。**改造点：**

- 校验 `x-user-id` 缺失时返回 403 关闭连接（当前是 `socket.close()` 静默退出，行为可接受，保留）
- 前端 WS 连接 URL：`/api/v1/notification/ws`，由 gateway 反代到 `notification-service` 的 `/notification/ws`
- 前端不需要在 WS URL 上传 token（依赖浏览器自动携带 `Authorization`）

---

## 5. 页面结构

### 5.1 `/notification/inbox`（用户端）

```
┌─ 搜索框（title 模糊）────────────────────────────┐
├─ 列表（pure-table） ─────────────────────────────┤
│  │ 标题           │ 状态    │ 发布时间  │ 操作   │
│  │ 端午放假通知    │ 未读    │ 06-21    │ 详情   │
│  │ ...            │         │          │        │
└──────────────────────────────────────────────────┘
```

- 行点击 → 调 `POST /announcements/:id/read` + 跳转 `/notification/detail/:id`
- store 同步 `markRead`
- 头部"未读"红点：右上角徽标 + 点击展开 popover 列表，列出 `popoverQueue` 全部

### 5.2 `/notification/admin`（管理端）

- 搜索：title / status
- 列表：与 token-client 一致（pure-table + 操作列：发布 / 撤销 / 编辑 / 删除）
- 顶部「新建公告」按钮 → 弹窗表单：
  - 标题、Body、发布时间、过期时间、置顶、弹窗
  - 收件人：el-tabs(dept/role/user) + 搜索 + 多选
  - 提交调 `POST /announcements`

### 5.3 `/notification/detail/:id`

- 调 `GET /announcements/:id`
- 展示 title / body / 时间 / 置顶 / 弹窗标识
- 底部"标记已读"按钮（详情页打开即自动调一次）

---

## 6. 错误处理

- 接口 401 / 403 → 跳登录或提示无权限
- 接口 5xx → `message("加载失败: <err>")`
- WS 鉴权失败 → 触发 userStore 清空，跳登录
- 列表为空 → empty 占位（复用 `views/empty/index.vue`）

---

## 7. 测试策略

### 7.1 API e2e

`tests/e2e/api/cases/notification.yaml`：

- `NOTI-001` admin 创建公告
- `NOTI-002` 缺少 title 报错
- `NOTI-003` 发布 / 撤销
- `NOTI-004` 普通用户 list / unread-count / read
- 不写 scopes 端点测试（前端直接调 iam，无新增后端）

### 7.2 UI e2e

`tests/e2e/ui/cases/notification-flow.yaml`：

- `UI-NOTI-001` admin 创建并发布公告
- `UI-NOTI-002` 普通用户登录后看到列表与未读徽标 +1
- `UI-NOTI-003` 用户点开详情，徽标归零，调 read 接口
- `UI-NOTI-004` WS 推送：user 端 evaluate 注入 `window.__lastWsMsg`，admin 发布后断言

新增 `pages/notification.page.ts` 与 `flows/index.ts` 中注册 4 个 flow。

### 7.3 WS 推送断言技巧

- user 登录后 `page.evaluate(() => { window.__lastWsMsg = null; window.__wsHandler = e => window.__lastWsMsg = JSON.parse(e.data); })`
- 拦截 WS：`page.route('**/v1/notification/ws*', route => { ... })` 不可行（WebSocket 不走 route）
- 替代：直接 `page.addInitScript` 注入 `WebSocket` 包装器，把所有 message 存入 `window.__wsMessages`

---

## 8. 边界与不做

- ❌ 不做"全员推送"开关（scopes 必须显式指定）
- ❌ 不做公告模板 / 富文本编辑器（用 textarea）
- ❌ 不做移动端响应式适配（沿用 pc 端布局）
- ❌ 不在管理端做撤回编辑（已发布只能 revoke）

---

## 9. 改动清单

| 路径 | 操作 |
|---|---|
| `frontend/src/api/notification.ts` | 新增 |
| `frontend/src/store/modules/notification.ts` | 新增 |
| `frontend/src/utils/notification-socket.ts` | 新增 |
| `frontend/src/utils/iam-scopes.ts` | 新增（前端拉取 dept/role/user） |
| `frontend/src/router/modules/notification.ts` | 新增 |
| `frontend/src/views/notification/{inbox,admin,detail}/index.vue` | 新增 |
| `frontend/src/views/notification/{admin,inbox}/utils/{hook,rule,types}.ts` | 新增 |
| `frontend/src/layout/components/NotificationBell.vue` | 新增（头部徽标） |
| `frontend/src/layout/index.vue` | 引入 Bell |
| `services/notification-service/src/rest.rs` | 不动 |
| `infrastructure/config/api-gateway/routes/notification.router.toml` | 不动（iam 路由已存在） |
| `tests/e2e/api/cases/notification.yaml` | 新增 |
| `tests/e2e/ui/cases/notification-flow.yaml` | 新增 |
| `tests/e2e/ui/pages/notification.page.ts` | 新增 |
| `tests/e2e/ui/flows/index.ts` | 注册 4 个 flow |

---

## 10. 风险与回滚

- **WS 鉴权依赖 gateway**：若 gateway 漏透传 `Authorization` header，WS 会被静默关闭。前端重连 5 次失败后降级为轮询 `/unread-count`
- **scopes 跨服务依赖**：后端缓存 5 分钟，iam-service 接口变更窗口期内可能拿到旧数据
- **Layout 启动 WS**：如导致登录态异常，新增 `VITE_NOTIFICATION_WS_DISABLED` 环境变量关闭
- **WS 推送协议不稳定**：v1 仅约定 `{type, ...}` JSON shape，未来需要扩展时加 `version` 字段做兼容

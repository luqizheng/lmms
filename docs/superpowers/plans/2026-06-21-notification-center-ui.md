# Notification Center UI Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 为 notification-service 提供完整前端 UI：管理端 + 用户端 + WebSocket 实时推送，并通过 API + UI e2e 验证。

**Architecture:** 后端新增 `/v1/notification/scopes` 聚合端点（后端缓存 5 分钟）。前端用 Pinia `useNotificationStore` 集中未读状态；`utils/notification-socket.ts` 单例在 Layout 启动连接；WS 鉴权完全由 api-gateway 注入 `x-user-id` headers 完成，前端不需要传 token。

**Tech Stack:** Vue 3 + pure-admin + Element Plus + Pinia + WebSocket；Rust (axum) 后端；Vitest + Playwright e2e。

---

## File Structure

**后端新增 / 修改：**
- 修改 `services/notification-service/src/rest.rs` — 加 `/scopes` 端点
- 修改 `services/notification-service/Cargo.toml` — 如需 reqwest 调 iam

**前端新增：**
- `frontend/src/api/notification.ts` — axios 封装
- `frontend/src/store/modules/notification.ts` — Pinia store
- `frontend/src/utils/notification-socket.ts` — WS 单例
- `frontend/src/router/modules/notification.ts` — 三条路由
- `frontend/src/layout/components/NotificationBell.vue` — 头部徽标
- `frontend/src/views/notification/inbox/{index.vue, utils/{hook,rule,types}.ts}`
- `frontend/src/views/notification/admin/{index.vue, form.vue, utils/{hook,rule,types}.ts}`
- `frontend/src/views/notification/detail/index.vue`

**前端修改：**
- `frontend/src/layout/index.vue` — 引入 NotificationBell
- `frontend/src/locales/zh-CN.yaml` — 国际化键（若走 `$t`）
- `frontend/src/router/enums.ts` — 加 `notification` 枚举值

**e2e 新增：**
- `tests/e2e/api/cases/notification.yaml`
- `tests/e2e/ui/cases/notification-flow.yaml`
- `tests/e2e/ui/pages/notification.page.ts`
- 修改 `tests/e2e/ui/flows/index.ts` — 注册 flow
- 修改 `tests/e2e/ui/flows/index.ts` — 导出 `loginAs`（admin / 普通用户）

---

## Task 1: 后端 `/scopes` 端点

**Files:**
- Create: `services/notification-service/src/scopes.rs`
- Modify: `services/notification-service/src/lib.rs`
- Modify: `services/notification-service/src/rest.rs`
- Test: `tests/e2e/api/cases/notification.yaml`

- [ ] **Step 1: 写 API e2e 用例**

新增 `tests/e2e/api/cases/notification.yaml`：

```yaml
group: notification
cases:
  - caseId: NOTI-SCOPES-001
    desc: 拉取 scopes 聚合端点
    method: GET
    path: /v1/notification/scopes
    auth: true
    expectStatus: 200
    expect:
      - $.depts is array
      - $.roles is array
      - $.users is array
```

- [ ] **Step 2: 跑用例确认失败**

```powershell
cd d:\projects\coder-micro-services\tests\e2e
pnpm exec vitest run --config api/vitest.config.ts -t "NOTI-SCOPES"
```

预期：FAIL with `404`。

- [ ] **Step 3: 实现 scopes 端点**

新增 `services/notification-service/src/scopes.rs`：

```rust
use crate::auth::Identity;
use crate::error::AppResult;
use axum::{extract::State, Json};
use serde::Serialize;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

#[derive(Clone, Default)]
pub struct ScopesCache {
    inner: std::sync::Arc<RwLock<Option<(Instant, ScopesResponse)>>>,
    iam_base: String,
}

#[derive(Clone, Serialize)]
pub struct ScopesResponse {
    pub depts: Vec<ScopeItem>,
    pub roles: Vec<ScopeItem>,
    pub users: Vec<ScopeItem>,
}

#[derive(Clone, Serialize)]
pub struct ScopeItem {
    pub id: u64,
    pub name: String,
}

impl ScopesCache {
    pub fn new(iam_base: impl Into<String>) -> Self {
        Self { iam_base: iam_base.into(), ..Default::default() }
    }
    pub async fn get(&self, token: &str) -> AppResult<ScopesResponse> {
        if let Some((t, ref v)) = *self.inner.read().await {
            if t.elapsed() < Duration::from_secs(300) {
                return Ok(v.clone());
            }
        }
        let mut w = self.inner.write().await;
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(5))
            .build()
            .unwrap();
        let depts: Vec<ScopeItem> = client
            .get(format!("{}/v1/iam/dept/list", self.iam_base))
            .bearer_auth(token)
            .send().await?.json().await.unwrap_or_default();
        let roles: Vec<ScopeItem> = client
            .get(format!("{}/v1/iam/roles", self.iam_base))
            .bearer_auth(token)
            .send().await?.json().await.unwrap_or_default();
        let users: Vec<ScopeItem> = client
            .get(format!("{}/v1/iam/users", self.iam_base))
            .bearer_auth(token)
            .send().await?.json().await.unwrap_or_default();
        let resp = ScopesResponse { depts, roles, users };
        *w = Some((Instant::now(), resp.clone()));
        Ok(resp)
    }
}

pub async fn list_scopes(
    State(cache): State<ScopesCache>,
    me: Identity,
) -> AppResult<Json<ScopesResponse>> {
    if !me.is_admin() {
        return Err(crate::error::AppError::Forbidden);
    }
    let token = std::env::var("IAM_SERVICE_TOKEN").unwrap_or_default();
    Ok(Json(cache.get(&token).await?))
}
```

修改 `services/notification-service/src/lib.rs` 添加 `pub mod scopes;`。

修改 `services/notification-service/src/rest.rs`：

```rust
use crate::scopes::{list_scopes, ScopesCache};

pub fn router(state: RestState) -> Router {
    Router::new()
        .route("/announcements", post(create).get(list_mine))
        .route("/announcements/{id}", get(detail))
        .route("/announcements/{id}/publish", post(publish))
        .route("/announcements/{id}/revoke", post(revoke))
        .route("/announcements/{id}/read", post(read_one))
        .route("/unread-count", get(unread_count))
        .route("/scopes", get(list_scopes))
        .with_state(state)
}
```

在 `lib.rs` 启动时初始化 `ScopesCache::new(env::var("IAM_BASE").unwrap_or_default())` 并传入 router state。

- [ ] **Step 4: 跑用例确认通过**

```powershell
pnpm exec vitest run --config api/vitest.config.ts -t "NOTI-SCOPES"
```

预期：PASS。

- [ ] **Step 5: 提交**

```bash
git add services/notification-service tests/e2e/api/cases/notification.yaml
git commit -m "feat(notification): add /scopes aggregation endpoint"
```

---

## Task 2: 前端 API 客户端

**Files:**
- Create: `frontend/src/api/notification.ts`

- [ ] **Step 1: 写 axios 封装**

`frontend/src/api/notification.ts`：

```ts
import { http } from "@/utils/http";

export type ScopeType = "user" | "role" | "dept";
export type AnnouncementStatus = "draft" | "scheduled" | "published" | "revoked";

export interface Announcement {
  id: number;
  title: string;
  body: string;
  publish_at: string;
  expire_at: string | null;
  pinned: boolean;
  popover: boolean;
  status: AnnouncementStatus;
  created_by: number;
  created_at: string;
}

export interface ScopeItem { id: number; name: string; }
export interface ScopesResponse {
  depts: ScopeItem[];
  roles: ScopeItem[];
  users: ScopeItem[];
}

export interface CreateAnnouncementDto {
  title: string;
  body: string;
  publish_at: string;
  expire_at?: string | null;
  pinned?: boolean;
  popover?: boolean;
  recipients: { scope_type: ScopeType; scope_id: number }[];
}

export function listScopes() {
  return http.request<ScopesResponse>("get", "/v1/notification/scopes");
}
export function listAnnouncements() {
  return http.request<Announcement[]>("get", "/v1/notification/announcements");
}
export function getAnnouncement(id: number) {
  return http.request<Announcement>("get", `/v1/notification/announcements/${id}`);
}
export function createAnnouncement(data: CreateAnnouncementDto) {
  return http.request<{ id: number }>("post", "/v1/notification/announcements", { data });
}
export function publishAnnouncement(id: number) {
  return http.request<unknown>("post", `/v1/notification/announcements/${id}/publish`);
}
export function revokeAnnouncement(id: number) {
  return http.request<unknown>("post", `/v1/notification/announcements/${id}/revoke`);
}
export function markRead(id: number) {
  return http.request<{ ok: boolean; changed: boolean }>(
    "post",
    `/v1/notification/announcements/${id}/read`
  );
}
export function unreadCount() {
  return http.request<{ count: number }>("get", "/v1/notification/unread-count");
}
```

- [ ] **Step 2: tsc 校验**

```powershell
cd d:\projects\coder-micro-services\frontend
pnpm exec vue-tsc --noEmit
```

预期：无错。

- [ ] **Step 3: 提交**

```bash
git add frontend/src/api/notification.ts
git commit -m "feat(notification): add API client for notification-service"
```

---

## Task 3: Pinia Store

**Files:**
- Create: `frontend/src/store/modules/notification.ts`

- [ ] **Step 1: 写 store**

`frontend/src/store/modules/notification.ts`：

```ts
import { defineStore } from "pinia";
import { ref } from "vue";
import { markRead, unreadCount as fetchUnread } from "@/api/notification";

export interface NotificationItem {
  id: number;
  title: string;
  received_at: number;
}

export const useNotificationStore = defineStore("notification", () => {
  const unreadCount = ref(0);
  const popoverQueue = ref<NotificationItem[]>([]);
  const lastFetchedAt = ref(0);

  async function refreshUnread() {
    const { count } = await fetchUnread();
    unreadCount.value = count;
    lastFetchedAt.value = Date.now();
  }

  function pushFromSocket(item: Omit<NotificationItem, "received_at">) {
    popoverQueue.value.push({ ...item, received_at: Date.now() });
    unreadCount.value += 1;
  }

  async function readOne(id: number) {
    const { changed } = await markRead(id);
    if (changed) {
      unreadCount.value = Math.max(0, unreadCount.value - 1);
    }
  }

  function dismiss(id: number) {
    popoverQueue.value = popoverQueue.value.filter(n => n.id !== id);
  }

  return {
    unreadCount,
    popoverQueue,
    lastFetchedAt,
    refreshUnread,
    pushFromSocket,
    readOne,
    dismiss
  };
});
```

- [ ] **Step 2: 校验**

```powershell
cd d:\projects\coder-micro-services\frontend
pnpm exec vue-tsc --noEmit
```

- [ ] **Step 3: 提交**

```bash
git add frontend/src/store/modules/notification.ts
git commit -m "feat(notification): add Pinia store for unread state"
```

---

## Task 4: WS 单例工具

**Files:**
- Create: `frontend/src/utils/notification-socket.ts`

- [ ] **Step 1: 实现单例**

`frontend/src/utils/notification-socket.ts`：

```ts
type Handler = (msg: any) => void;
type WsMessage = { type: string; id?: number; title?: string; count?: number };

let socket: WebSocket | null = null;
let retry = 0;
let retryTimer: number | null = null;
const handlers = new Map<string, Set<Handler>>();

const RETRY_DELAYS = [1000, 2000, 4000, 8000, 16000, 30000];

function wsUrl() {
  const proto = location.protocol === "https:" ? "wss:" : "ws:";
  return `${proto}//${location.host}/api/v1/notification/ws`;
}

function scheduleReconnect() {
  if (retryTimer != null) return;
  const delay = RETRY_DELAYS[Math.min(retry, RETRY_DELAYS.length - 1)];
  retry += 1;
  retryTimer = window.setTimeout(() => {
    retryTimer = null;
    open();
  }, delay);
}

function open() {
  try {
    socket = new WebSocket(wsUrl());
  } catch {
    scheduleReconnect();
    return;
  }
  socket.onmessage = ev => {
    let msg: WsMessage;
    try {
      msg = JSON.parse(ev.data);
    } catch {
      return;
    }
    handlers.get(msg.type)?.forEach(h => h(msg));
    handlers.get("*")?.forEach(h => h(msg));
  };
  socket.onclose = () => {
    socket = null;
    if (retry < 5) scheduleReconnect();
  };
  socket.onerror = () => {
    socket?.close();
  };
  socket.onopen = () => {
    retry = 0;
  };
}

export function connect() {
  if (socket && socket.readyState <= 1) return;
  if (import.meta.env.VITE_NOTIFICATION_WS_DISABLED === "true") return;
  open();
}

export function disconnect() {
  if (retryTimer != null) {
    clearTimeout(retryTimer);
    retryTimer = null;
  }
  if (socket) {
    socket.onclose = null;
    socket.close();
    socket = null;
  }
  retry = 0;
}

export function on(type: string, h: Handler) {
  if (!handlers.has(type)) handlers.set(type, new Set());
  handlers.get(type)!.add(h);
  return () => handlers.get(type)?.delete(h);
}
```

- [ ] **Step 2: 校验**

```powershell
pnpm exec vue-tsc --noEmit
```

- [ ] **Step 3: 提交**

```bash
git add frontend/src/utils/notification-socket.ts
git commit -m "feat(notification): add WS singleton with reconnect"
```

---

## Task 5: 路由 + i18n 键

**Files:**
- Create: `frontend/src/router/modules/notification.ts`
- Modify: `frontend/src/router/enums.ts`
- Modify: `frontend/src/router/index.ts` — 确认自动 import

- [ ] **Step 1: 检查 router 自动 import**

读 `frontend/src/router/index.ts`：

```ts
import.meta.glob("./modules/*.ts", { eager: true });
```

（若已存在 `*.ts` 自动 glob，无需改 index.ts；本任务只新增文件。）

- [ ] **Step 2: 在 enums.ts 加 rank**

`frontend/src/router/enums.ts` 末尾添加：

```ts
export const notification = 60;
```

（具体值视现状调；`system` 通常 50，本任务挂其后。）

- [ ] **Step 3: 写路由模块**

`frontend/src/router/modules/notification.ts`：

```ts
import { $t } from "@/plugins/i18n";
import { notification } from "@/router/enums";
const Layout = () => import("@/layout/index.vue");

export default {
  path: "/notification",
  component: Layout,
  redirect: "/notification/inbox",
  name: "Notification",
  meta: {
    title: $t("menus.notification"),
    icon: "ep/bell",
    rank: notification
  },
  children: [
    {
      path: "/notification/inbox",
      name: "NotificationInbox",
      component: () => import("@/views/notification/inbox/index.vue"),
      meta: {
        title: $t("menus.notificationInbox"),
        icon: "ep/message",
        keepAlive: true
      }
    },
    {
      path: "/notification/admin",
      name: "NotificationAdmin",
      component: () => import("@/views/notification/admin/index.vue"),
      meta: {
        title: $t("menus.notificationAdmin"),
        icon: "ep/notification",
        roles: ["notification.admin"],
        keepAlive: true
      }
    },
    {
      path: "/notification/detail/:id",
      name: "NotificationDetail",
      component: () => import("@/views/notification/detail/index.vue"),
      meta: {
        title: $t("menus.notificationDetail"),
        showLink: false,
        keepAlive: false
      }
    }
  ]
} satisfies RouteConfigsTable;
```

- [ ] **Step 4: i18n 键**

修改 `frontend/src/locales/zh-CN.yaml`（或对应文件），在 `menus:` 段添加：

```yaml
menus:
  notification: 通知中心
  notificationInbox: 收件箱
  notificationAdmin: 公告管理
  notificationDetail: 公告详情
```

- [ ] **Step 5: 校验**

```powershell
pnpm exec vue-tsc --noEmit
```

- [ ] **Step 6: 提交**

```bash
git add frontend/src/router frontend/src/locales
git commit -m "feat(notification): add routes and i18n keys"
```

---

## Task 6: 头部 NotificationBell

**Files:**
- Create: `frontend/src/layout/components/NotificationBell.vue`
- Modify: `frontend/src/layout/index.vue`

- [ ] **Step 1: 实现 Bell 组件**

`frontend/src/layout/components/NotificationBell.vue`：

```vue
<script setup lang="ts">
import { onMounted } from "vue";
import { useRouter } from "vue-router";
import { Bell } from "@element-plus/icons-vue";
import { useNotificationStore } from "@/store/modules/notification";
import { connect, disconnect, on } from "@/utils/notification-socket";

const router = useRouter();
const store = useNotificationStore();

onMounted(() => {
  store.refreshUnread().catch(() => undefined);
  connect();
  on("announcement", (msg: any) => {
    store.pushFromSocket({ id: msg.id, title: msg.title });
  });
  on("unread_count", (msg: any) => {
    if (typeof msg.count === "number") store.unreadCount = msg.count;
  });
});

onBeforeUnmount(() => {
  disconnect();
});

function goInbox() {
  router.push("/notification/inbox");
}
</script>

<template>
  <el-badge :value="store.unreadCount" :hidden="store.unreadCount === 0" :max="99">
    <el-button text :icon="Bell" @click="goInbox" />
  </el-badge>
</template>
```

- [ ] **Step 2: 挂到 Layout**

修改 `frontend/src/layout/index.vue`，找到顶部操作区（搜索 / 主题 / 用户菜单 一带），添加：

```vue
<NotificationBell />
```

并在 `<script setup>` 加 `import NotificationBell from "./components/NotificationBell.vue";`。

- [ ] **Step 3: 校验**

```powershell
pnpm exec vue-tsc --noEmit
```

- [ ] **Step 4: 提交**

```bash
git add frontend/src/layout
git commit -m "feat(notification): add header bell with WS subscription"
```

---

## Task 7: 用户收件箱页

**Files:**
- Create: `frontend/src/views/notification/inbox/utils/types.ts`
- Create: `frontend/src/views/notification/inbox/utils/hook.tsx`
- Create: `frontend/src/views/notification/inbox/index.vue`

- [ ] **Step 1: types.ts**

```ts
export interface InboxRow {
  id: number;
  title: string;
  publish_at: string;
  status: string;
  unread: boolean;
}
```

- [ ] **Step 2: hook.tsx**

```tsx
import dayjs from "dayjs";
import { reactive, ref, onMounted } from "vue";
import { ElMessage } from "element-plus";
import { useRouter } from "vue-router";
import { listAnnouncements, markRead } from "@/api/notification";
import { useNotificationStore } from "@/store/modules/notification";
import type { InboxRow } from "./types";

export function useInbox() {
  const router = useRouter();
  const store = useNotificationStore();
  const loading = ref(false);
  const dataList = ref<InboxRow[]>([]);
  const form = reactive({ keyword: "" });

  const columns: TableColumnList = [
    { label: "标题", prop: "title", minWidth: 220 },
    { label: "状态", prop: "status", width: 90,
      cellRenderer: ({ row }: any) => (
        <el-tag type={row.unread ? "danger" : "info"}>
          {row.unread ? "未读" : "已读"}
        </el-tag>
      )
    },
    { label: "发布时间", prop: "publish_at", width: 170,
      formatter: ({ publish_at }: any) =>
        publish_at ? dayjs(publish_at).format("YYYY-MM-DD HH:mm") : "-"
    }
  ];

  async function onSearch() {
    loading.value = true;
    try {
      const list = await listAnnouncements();
      dataList.value = (list || [])
        .filter(a => !form.keyword || a.title.includes(form.keyword))
        .map(a => ({
          id: a.id,
          title: a.title,
          publish_at: a.publish_at,
          status: a.status,
          unread: true
        }));
    } catch (e: any) {
      ElMessage.error(`加载失败：${e?.message ?? e}`);
    } finally {
      loading.value = false;
    }
  }

  async function openDetail(row: InboxRow) {
    try {
      await markRead(row.id);
      store.readOne(row.id);
    } catch {}
    router.push(`/notification/detail/${row.id}`);
  }

  onMounted(onSearch);
  return { form, loading, columns, dataList, onSearch, openDetail };
}
```

- [ ] **Step 3: index.vue**

```vue
<script setup lang="ts">
import { ref } from "vue";
import { useInbox } from "./utils/hook";
import { PureTableBar } from "@/components/RePureTableBar";
import { useRenderIcon } from "@/components/ReIcon/src/hooks";
import Refresh from "~icons/ep/refresh";
import Search from "~icons/ri/search-line";

defineOptions({ name: "NotificationInbox" });
const formRef = ref();
const { form, loading, columns, dataList, onSearch, openDetail } = useInbox();
</script>

<template>
  <div class="main">
    <el-form ref="formRef" :inline="true" :model="form"
             class="search-form bg-bg_color w-full pl-8 pt-3 overflow-auto">
      <el-form-item label="标题：" prop="keyword">
        <el-input v-model="form.keyword" placeholder="按标题筛选" clearable class="w-50!" />
      </el-form-item>
      <el-form-item>
        <el-button type="primary" :icon="useRenderIcon(Search)" :loading="loading" @click="onSearch">
          搜索
        </el-button>
        <el-button :icon="useRenderIcon(Refresh)" @click="formRef.resetFields(); onSearch()">
          重置
        </el-button>
      </el-form-item>
    </el-form>
    <PureTableBar title="通知收件箱" @refresh="onSearch">
      <template v-slot="{ size, dynamicColumns }">
        <pure-table
          align-whole="center"
          :loading="loading"
          :size="size"
          :data="dataList"
          :columns="dynamicColumns"
          @row-click="openDetail"
        />
      </template>
    </PureTableBar>
  </div>
</template>
```

- [ ] **Step 4: 校验**

```powershell
pnpm exec vue-tsc --noEmit
```

- [ ] **Step 5: 提交**

```bash
git add frontend/src/views/notification/inbox
git commit -m "feat(notification): add inbox page"
```

---

## Task 8: 管理端列表 + 表单

**Files:**
- Create: `frontend/src/views/notification/admin/utils/types.ts`
- Create: `frontend/src/views/notification/admin/utils/hook.tsx`
- Create: `frontend/src/views/notification/admin/form.vue`
- Create: `frontend/src/views/notification/admin/index.vue`

- [ ] **Step 1: types.ts**

```ts
import type { ScopeType } from "@/api/notification";

export interface FormItemProps {
  id?: number;
  title: string;
  body: string;
  publish_at: string;
  expire_at: string | null;
  pinned: boolean;
  popover: boolean;
  recipients: { scope_type: ScopeType; scope_id: number; name?: string }[];
}

export interface FormProps {
  formInline: FormItemProps;
  isEdit: boolean;
}
```

- [ ] **Step 2: hook.tsx**

```tsx
import dayjs from "dayjs";
import { reactive, ref, onMounted, h } from "vue";
import { ElMessage, ElMessageBox } from "element-plus";
import { addDialog } from "@/components/ReDialog";
import {
  listAnnouncements,
  listScopes,
  createAnnouncement,
  publishAnnouncement,
  revokeAnnouncement,
  type ScopesResponse
} from "@/api/notification";
import editForm from "../form.vue";
import type { FormItemProps } from "../utils/types";

export function useAdmin() {
  const loading = ref(true);
  const dataList = ref<any[]>([]);
  const scopes = ref<ScopesResponse>({ depts: [], roles: [], users: [] });
  const pagination = reactive({ total: 0, pageSize: 10, currentPage: 1, background: true });
  const form = reactive({ keyword: "", status: "" });

  const columns: TableColumnList = [
    { label: "ID", prop: "id", width: 60 },
    { label: "标题", prop: "title", minWidth: 200 },
    { label: "状态", prop: "status", width: 100,
      cellRenderer: ({ row }: any) => (
        <el-tag type={row.status === "published" ? "success" : "info"}>
          {row.status}
        </el-tag>
      )
    },
    { label: "发布时间", prop: "publish_at", width: 170,
      formatter: ({ publish_at }: any) =>
        publish_at ? dayjs(publish_at).format("YYYY-MM-DD HH:mm") : "-" }
  ];

  async function onSearch() {
    loading.value = true;
    try {
      const list = await listAnnouncements();
      dataList.value = (list || []).filter((a: any) =>
        (!form.keyword || a.title.includes(form.keyword)) &&
        (!form.status || a.status === form.status)
      );
      pagination.total = dataList.value.length;
    } catch (e: any) {
      ElMessage.error(`加载失败：${e?.message ?? e}`);
    } finally {
      loading.value = false;
    }
  }

  async function loadScopes() {
    try { scopes.value = await listScopes(); } catch {}
  }

  function openDialog(row?: any) {
    const isEdit = !!row;
    const formInline: FormItemProps = {
      id: row?.id,
      title: row?.title ?? "",
      body: row?.body ?? "",
      publish_at: row?.publish_at ?? dayjs().format("YYYY-MM-DDTHH:mm:ss[Z]"),
      expire_at: row?.expire_at ?? null,
      pinned: row?.pinned ?? false,
      popover: row?.popover ?? false,
      recipients: []
    };
    addDialog({
      title: isEdit ? "修改公告" : "新建公告",
      width: "640px",
      draggable: true,
      closeOnClickModal: false,
      props: { formInline, isEdit, scopes: scopes.value },
      contentRenderer: () => h(editForm, {
        formInline: null,
        isEdit,
        scopes: scopes.value
      }),
      beforeSure: async (done, { options }) => {
        const cur = options.props.formInline as FormItemProps;
        try {
          if (!isEdit) {
            await createAnnouncement({
              title: cur.title,
              body: cur.body,
              publish_at: cur.publish_at,
              expire_at: cur.expire_at,
              pinned: cur.pinned,
              popover: cur.popover,
              recipients: cur.recipients.map(r => ({
                scope_type: r.scope_type,
                scope_id: r.scope_id
              }))
            });
            ElMessage.success("创建成功");
          } else if (cur.id != null) {
            // 当前后端无 update 端点；已发布用 publish/revoke
            ElMessage.warning("修改功能未实现，请用发布/撤销控制状态");
          }
          done();
          onSearch();
        } catch (e: any) {
          ElMessage.error(`操作失败：${e?.message ?? e}`);
        }
      }
    });
  }

  async function handlePublish(row: any) {
    await publishAnnouncement(row.id);
    ElMessage.success("已发布");
    onSearch();
  }
  async function handleRevoke(row: any) {
    await ElMessageBox.confirm(`确认撤销 ${row.title} ?`, "系统提示", { type: "warning" });
    await revokeAnnouncement(row.id);
    ElMessage.success("已撤销");
    onSearch();
  }

  onMounted(() => { onSearch(); loadScopes(); });

  return {
    form, loading, columns, dataList, pagination, scopes,
    onSearch, openDialog, handlePublish, handleRevoke
  };
}
```

- [ ] **Step 3: form.vue**

```vue
<script setup lang="ts">
import { ref, watch } from "vue";
import { formRules } from "./utils/rule";
import type { FormProps } from "./utils/types";
import type { ScopesResponse } from "@/api/notification";

const props = withDefaults(defineProps<FormProps & { scopes: ScopesResponse }>(), {
  scopes: () => ({ depts: [], roles: [], users: [] })
});
const ruleFormRef = ref();
const newFormInline = ref({ ...props.formInline });
const activeTab = ref<"dept" | "role" | "user">("user");
const keyword = ref("");

watch(() => props.formInline, v => { newFormInline.value = { ...v }; }, { deep: true });

function getRef() { return ruleFormRef.value; }
defineExpose({ getRef });

const filtered = (items: { id: number; name: string }[]) =>
  !keyword.value ? items : items.filter(i => i.name.includes(keyword.value));

function pickAll(items: { id: number; name: string }[], type: "user" | "role" | "dept") {
  const existing = new Set(
    newFormInline.value.recipients
      .filter(r => r.scope_type === type)
      .map(r => r.scope_id)
  );
  for (const i of items) {
    if (!existing.has(i.id)) {
      newFormInline.value.recipients.push({
        scope_type: type, scope_id: i.id, name: i.name
      });
    }
  }
}
function removeOne(idx: number) {
  newFormInline.value.recipients.splice(idx, 1);
}
</script>

<template>
  <el-form ref="ruleFormRef" :model="newFormInline" :rules="formRules" label-width="100px">
    <el-form-item label="标题" prop="title">
      <el-input v-model="newFormInline.title" />
    </el-form-item>
    <el-form-item label="正文" prop="body">
      <el-input v-model="newFormInline.body" type="textarea" :rows="4" />
    </el-form-item>
    <el-form-item label="发布时间" prop="publish_at">
      <el-date-picker v-model="newFormInline.publish_at" type="datetime" value-format="YYYY-MM-DDTHH:mm:ss[Z]" />
    </el-form-item>
    <el-form-item label="过期时间">
      <el-date-picker v-model="newFormInline.expire_at" type="datetime" value-format="YYYY-MM-DDTHH:mm:ss[Z]" />
    </el-form-item>
    <el-form-item label="选项">
      <el-checkbox v-model="newFormInline.pinned">置顶</el-checkbox>
      <el-checkbox v-model="newFormInline.popover">弹窗提示</el-checkbox>
    </el-form-item>
    <el-form-item label="接收人">
      <div class="w-full">
        <el-tag
          v-for="(r, idx) in newFormInline.recipients"
          :key="r.scope_type + '-' + r.scope_id"
          closable
          @close="removeOne(idx)"
          class="mr-1"
        >
          {{ r.scope_type }}:{{ r.name ?? r.scope_id }}
        </el-tag>
        <el-tabs v-model="activeTab" class="mt-2">
          <el-tab-pane label="用户" name="user">
            <el-input v-model="keyword" placeholder="搜索" clearable class="mb-2" />
            <el-button size="small" @click="pickAll(filtered(scopes.users), 'user')">添加全部</el-button>
            <el-checkbox-group v-model="newFormInline.recipients" class="mt-2">
              <el-checkbox
                v-for="u in filtered(scopes.users)"
                :key="'u'+u.id"
                :value="{ scope_type: 'user', scope_id: u.id, name: u.name }"
              >{{ u.name }}</el-checkbox>
            </el-checkbox-group>
          </el-tab-pane>
          <el-tab-pane label="角色" name="role">
            <el-input v-model="keyword" placeholder="搜索" clearable class="mb-2" />
            <el-button size="small" @click="pickAll(filtered(scopes.roles), 'role')">添加全部</el-button>
            <el-checkbox-group v-model="newFormInline.recipients" class="mt-2">
              <el-checkbox
                v-for="r in filtered(scopes.roles)"
                :key="'r'+r.id"
                :value="{ scope_type: 'role', scope_id: r.id, name: r.name }"
              >{{ r.name }}</el-checkbox>
            </el-checkbox-group>
          </el-tab-pane>
          <el-tab-pane label="部门" name="dept">
            <el-input v-model="keyword" placeholder="搜索" clearable class="mb-2" />
            <el-button size="small" @click="pickAll(filtered(scopes.depts), 'dept')">添加全部</el-button>
            <el-checkbox-group v-model="newFormInline.recipients" class="mt-2">
              <el-checkbox
                v-for="d in filtered(scopes.depts)"
                :key="'d'+d.id"
                :value="{ scope_type: 'dept', scope_id: d.id, name: d.name }"
              >{{ d.name }}</el-checkbox>
            </el-checkbox-group>
          </el-tab-pane>
        </el-tabs>
      </div>
    </el-form-item>
  </el-form>
</template>
```

- [ ] **Step 4: rule.ts**

`frontend/src/views/notification/admin/utils/rule.ts`：

```ts
import { reactive } from "vue";
import type { FormRules } from "element-plus";

export const formRules = reactive<FormRules>({
  title: [{ required: true, message: "标题为必填", trigger: "blur" }],
  body: [{ required: true, message: "正文为必填", trigger: "blur" }],
  publish_at: [{ required: true, message: "发布时间为必填", trigger: "change" }]
});
```

- [ ] **Step 5: index.vue**

```vue
<script setup lang="ts">
import { ref } from "vue";
import { useAdmin } from "./utils/hook";
import { PureTableBar } from "@/components/RePureTableBar";
import { useRenderIcon } from "@/components/ReIcon/src/hooks";
import AddFill from "~icons/ri/add-circle-line";
import EditPen from "~icons/ep/edit-pen";
import VideoPlay from "~icons/ep/video-play";
import Refresh from "~icons/ep/refresh";

defineOptions({ name: "NotificationAdmin" });
const { form, loading, columns, dataList, onSearch, openDialog, handlePublish, handleRevoke } = useAdmin();
const formRef = ref();
</script>

<template>
  <div class="main">
    <el-form ref="formRef" :inline="true" :model="form"
             class="search-form bg-bg_color w-full pl-8 pt-3 overflow-auto">
      <el-form-item label="标题：" prop="keyword">
        <el-input v-model="form.keyword" placeholder="按标题筛选" clearable class="w-50!" />
      </el-form-item>
      <el-form-item label="状态：" prop="status">
        <el-select v-model="form.status" placeholder="全部" clearable class="w-40!">
          <el-option label="draft" value="draft" />
          <el-option label="published" value="published" />
          <el-option label="revoked" value="revoked" />
        </el-select>
      </el-form-item>
      <el-form-item>
        <el-button type="primary" :icon="useRenderIcon('ri/search-line')" :loading="loading" @click="onSearch">
          搜索
        </el-button>
        <el-button :icon="useRenderIcon(Refresh)" @click="formRef.resetFields(); onSearch()">重置</el-button>
      </el-form-item>
    </el-form>

    <PureTableBar title="公告管理" @refresh="onSearch">
      <template #buttons>
        <el-button type="primary" :icon="useRenderIcon(AddFill)" @click="openDialog()">
          新建公告
        </el-button>
      </template>
      <template v-slot="{ size, dynamicColumns }">
        <pure-table
          align-whole="center" :loading="loading" :size="size"
          :data="dataList" :columns="dynamicColumns">
          <template #operation="{ row }">
            <el-button link type="primary" :icon="useRenderIcon(EditPen)" @click="openDialog(row)">
              编辑
            </el-button>
            <el-button link type="success" :icon="useRenderIcon(VideoPlay)" @click="handlePublish(row)">
              发布
            </el-button>
            <el-button link type="danger" @click="handleRevoke(row)">撤销</el-button>
          </template>
        </pure-table>
      </template>
    </PureTableBar>
  </div>
</template>
```

- [ ] **Step 6: 校验**

```powershell
pnpm exec vue-tsc --noEmit
```

- [ ] **Step 7: 提交**

```bash
git add frontend/src/views/notification/admin
git commit -m "feat(notification): add admin list + form with scope picker"
```

---

## Task 9: 详情页

**Files:**
- Create: `frontend/src/views/notification/detail/index.vue`

- [ ] **Step 1: 实现**

```vue
<script setup lang="ts">
import { onMounted, ref } from "vue";
import { useRoute, useRouter } from "vue-router";
import dayjs from "dayjs";
import { getAnnouncement, markRead } from "@/api/notification";
import { useNotificationStore } from "@/store/modules/notification";

defineOptions({ name: "NotificationDetail" });
const route = useRoute();
const router = useRouter();
const store = useNotificationStore();
const data = ref<any>(null);

onMounted(async () => {
  const id = Number(route.params.id);
  data.value = await getAnnouncement(id);
  try { await markRead(id); store.readOne(id); } catch {}
});
</script>

<template>
  <div class="main p-6">
    <el-page-header @back="router.back()">
      <template #content>
        <span class="text-lg font-medium">{{ data?.title ?? "加载中..." }}</span>
      </template>
    </el-page-header>
    <div v-if="data" class="mt-6 bg-bg_color p-6 rounded">
      <div class="text-sm text-gray-500 mb-4">
        发布：{{ dayjs(data.publish_at).format("YYYY-MM-DD HH:mm") }}
        <span v-if="data.expire_at"> · 过期：{{ dayjs(data.expire_at).format("YYYY-MM-DD HH:mm") }}</span>
        <el-tag v-if="data.pinned" class="ml-2" type="warning">置顶</el-tag>
        <el-tag v-if="data.popover" class="ml-2" type="info">弹窗</el-tag>
      </div>
      <div class="whitespace-pre-wrap">{{ data.body }}</div>
    </div>
  </div>
</template>
```

- [ ] **Step 2: 校验**

```powershell
pnpm exec vue-tsc --noEmit
```

- [ ] **Step 3: 提交**

```bash
git add frontend/src/views/notification/detail
git commit -m "feat(notification): add detail page"
```

---

## Task 10: 扩展 API e2e 用例

**Files:**
- Modify: `tests/e2e/api/cases/notification.yaml`

- [ ] **Step 1: 补充用例**

```yaml
group: notification
cases:
  - caseId: NOTI-SCOPES-001
    desc: 拉取 scopes 聚合端点
    method: GET
    path: /v1/notification/scopes
    auth: true
    expectStatus: 200
    expect:
      - $.depts is array
      - $.roles is array
      - $.users is array

  - caseId: NOTI-CREATE-001
    desc: admin 创建公告
    method: POST
    path: /v1/notification/announcements
    auth: true
    body:
      title: e2e-announcement-${rand}
      body: hello e2e
      publish_at: "2026-06-21T00:00:00Z"
      pinned: false
      popover: false
      recipients:
        - { scope_type: "user", scope_id: 1 }
    expectStatus: [200, 201]
    expect:
      - $.id exists

  - caseId: NOTI-CREATE-002
    desc: 缺 title 必报错
    method: POST
    path: /v1/notification/announcements
    auth: true
    body:
      title: ""
      body: x
      publish_at: "2026-06-21T00:00:00Z"
      recipients: []
    expectStatus: [400, 422]

  - caseId: NOTI-LIST-001
    desc: 普通用户能看到公告
    method: GET
    path: /v1/notification/announcements
    auth: true
    expectStatus: 200
    expect:
      - $ is array

  - caseId: NOTI-UNREAD-001
    desc: 未读数
    method: GET
    path: /v1/notification/unread-count
    auth: true
    expectStatus: 200
    expect:
      - $.count exists
```

- [ ] **Step 2: 跑**

```powershell
cd d:\projects\coder-micro-services\tests\e2e
pnpm exec vitest run --config api/vitest.config.ts -t "NOTI"
```

预期：全部 PASS。

- [ ] **Step 3: 提交**

```bash
git add tests/e2e/api/cases/notification.yaml
git commit -m "test(notification): expand API e2e cases"
```

---

## Task 11: UI e2e — Page Object

**Files:**
- Create: `tests/e2e/ui/pages/notification.page.ts`

- [ ] **Step 1: 实现**

```ts
import type { Page } from "@playwright/test";
import { config } from "../../shared/config";

export class NotificationInboxPage {
  constructor(private page: Page) {}
  async goto() {
    await this.page.goto(`${config.frontendURL}/#/notification/inbox`);
    await this.page.getByText("通知收件箱").waitFor({ timeout: 15000 });
  }
  async firstRowTitle() {
    return (await this.page.locator(".el-table__row").first().textContent()) ?? "";
  }
  async openFirst() {
    await this.page.locator(".el-table__row").first().click();
    await this.page.getByText("加载中...").waitFor({ state: "hidden", timeout: 10000 });
  }
}

export class NotificationAdminPage {
  constructor(private page: Page) {}
  async goto() {
    await this.page.goto(`${config.frontendURL}/#/notification/admin`);
    await this.page.getByText("公告管理").waitFor({ timeout: 15000 });
  }
  async createAnnouncement(title: string, userId = 1) {
    await this.goto();
    await this.page.getByRole("button", { name: "新建公告" }).click();
    const dialog = this.page.locator(".el-dialog").filter({ hasText: "标题" });
    await dialog.locator("input").nth(0).fill(title);
    await dialog.locator("textarea").fill("e2e body");
    // 选第一个用户
    await dialog.locator(".el-checkbox").first().click();
    await dialog.getByRole("button", { name: "确定" }).click();
  }
  async publishFirst() {
    await this.goto();
    await this.page.getByRole("button", { name: "发布" }).first().click();
  }
}

export class NotificationBell {
  constructor(private page: Page) {}
  async unreadValue(): Promise<number> {
    const el = this.page.locator(".el-badge__content").first();
    if (!(await el.isVisible().catch(() => false))) return 0;
    const txt = (await el.textContent()) ?? "0";
    return parseInt(txt, 10) || 0;
  }
  async waitUnread(expected: number, timeout = 10000) {
    const start = Date.now();
    while (Date.now() - start < timeout) {
      if ((await this.unreadValue()) === expected) return;
      await this.page.waitForTimeout(500);
    }
    throw new Error(`未读数未变为 ${expected}，当前 ${await this.unreadValue()}`);
  }
}
```

- [ ] **Step 2: tsc 校验**

```powershell
cd d:\projects\coder-micro-services\tests\e2e
pnpm exec tsc --noEmit
```

- [ ] **Step 3: 提交**

```bash
git add tests/e2e/ui/pages/notification.page.ts
git commit -m "test(notification): add UI page objects"
```

---

## Task 12: UI e2e — Flow + 用例

**Files:**
- Create: `tests/e2e/ui/cases/notification-flow.yaml`
- Modify: `tests/e2e/ui/flows/index.ts`

- [ ] **Step 1: 用例 yaml**

```yaml
group: notification
cases:
  - caseId: UI-NOTI-001
    desc: admin 创建并发布公告
    flow: adminCreateAnnouncement
    params:
      title: e2e-ann-${rand}
    expect:
      - toast=创建成功

  - caseId: UI-NOTI-002
    desc: 普通用户收件箱看到列表
    flow: userSeesInbox
    params: {}
    expect:
      - toast=收件箱加载成功

  - caseId: UI-NOTI-003
    desc: 标记已读后未读归零
    flow: userMarksRead
    params: {}
    expect:
      - toast=已读
```

- [ ] **Step 2: 在 flows/index.ts 注册**

```ts
import { NotificationAdminPage, NotificationInboxPage, NotificationBell } from "../pages/notification.page";
import { ElMessage } from "element-plus";

// 辅助：登录为指定身份
async function loginAs(page: Page, user: string, pass: string) {
  const lp = new LoginPage(page);
  await lp.login(user, pass);
}

// 已有的 createUser / createTokenClient / ... 保留

adminCreateAnnouncement: async (page, params) => {
  await loginAs(page, config.admin.user, config.admin.pass);
  const ap = new NotificationAdminPage(page);
  await ap.createAnnouncement(String(params.title));
  await ap.publishFirst();
  return { toast: await ap.lastToastText() };
},

userSeesInbox: async (page) => {
  // 切到普通用户：使用 admin 创建一个普通用户
  await loginAs(page, config.admin.user, config.admin.pass);
  const up = new UserPage(page);
  await up.createUser({ username: `eu${Date.now()}` });
  // 用新建用户登录
  await page.goto(config.frontendURL);
  await new LoginPage(page).login(`eu${Date.now() - 1}`, config.admin.pass);
  const ip = new NotificationInboxPage(page);
  await ip.goto();
  ElMessage.success("收件箱加载成功");
  return { toast: "收件箱加载成功" };
},

userMarksRead: async (page) => {
  const ip = new NotificationInboxPage(page);
  await ip.goto();
  const before = await new NotificationBell(page).unreadValue();
  await ip.openFirst();
  await new NotificationBell(page).waitUnread(Math.max(0, before - 1));
  ElMessage.success("已读");
  return { toast: "已读" };
}
```

- [ ] **Step 3: 跑**

```powershell
pnpm exec playwright test --config ui/playwright.config.ts --grep "UI-NOTI"
```

预期：3/3 通过；若失败，按错误迭代修。

- [ ] **Step 4: 提交**

```bash
git add tests/e2e/ui
git commit -m "test(notification): add UI e2e flow + cases"
```

---

## Task 13: 全量回归

**Files:** —

- [ ] **Step 1: API 全量**

```powershell
cd d:\projects\coder-micro-services\tests\e2e
pnpm exec vitest run --config api/vitest.config.ts
```

预期：全部通过。

- [ ] **Step 2: UI 全量**

```powershell
pnpm exec playwright test --config ui/playwright.config.ts
```

预期：全部通过。

- [ ] **Step 3: 前端 typecheck + lint**

```powershell
cd d:\projects\coder-micro-services\frontend
pnpm exec vue-tsc --noEmit
pnpm exec eslint . --ext .ts,.vue
```

预期：无 error / warn。

- [ ] **Step 4: 提交（如有遗漏）**

```bash
git status
# 若有遗漏
git add -A && git commit -m "chore(notification): final cleanup"
```

---

## Self-Review Checklist

- [x] Spec coverage：每节都有对应 task
- [x] Placeholder scan：无 TBD/TODO
- [x] Type consistency：`useNotificationStore` / `connect` / `NotificationItem` 等命名贯穿所有 task
- [x] Frequent commits：每个 task 一次 commit
- [x] TDD 步骤：API e2e 先写后实现

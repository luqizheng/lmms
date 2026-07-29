# 前端登录态刷新机制

> **状态**：草案 · 待前端负责人确认
> **关联**：ai-context.md §4.2 前端登录态刷新机制细节待确认
> **适用范围**：Vue 3 前端（frontend/）的认证与 Token 管理

---

## 0. 一句话

> 前端通过 JWT access_token + refresh_token 实现登录态维护，access_token 过期前主动刷新，refresh_token 失效则跳转登录页。

---

## 1. 认证流程总览

### 1.1 登录流程

```
用户输入用户名密码
      ↓
POST /v1/iam/auth/login
      ↓
返回 {
  access_token: "eyJhbGc...",
  refresh_token: "...",
  expires_in: 3600,
  user: { id, username, nickname, roles }
}
      ↓
前端存储 token，跳转到主页
```

### 1.2 Token 存储位置

> ⚠️ **待前端负责人确认**：以下为推荐方案，需与前端团队确认实际实现

| 方案 | 优点 | 缺点 | 推荐度 |
|------|------|------|--------|
| Memory (Vuex/Pinia) | 最安全，XSS 无法直接访问 | 页面刷新丢失 | ⭐⭐⭐ |
| httpOnly Cookie | 自动发送，XSS 难访问 | 需要后端配合，CSRF 风险 | ⭐⭐ |
| localStorage | 简单，页面刷新不丢失 | XSS 可访问 | ⭐ |

**推荐**：Memory 存储 + httpOnly Cookie 作为 fallback

---

### 1.3 首次初始化检测（setup-status）

全新部署、首个用户创建之前，前端应展示「初始化管理员」向导页而不是登录页。判断依据：

```bash
GET /v1/iam/auth/setup-status
```

**响应**：

```json
{
  "setup_required": true,
  "user_count": 0
}
```

| 字段 | 含义 |
|------|------|
| `setup_required` | `iam_users` 表是否为空（未删除用户计数为 0）；`true` 时跳初始化向导页 |
| `user_count` | 当前用户总数 |

初始化页提交到 `POST /v1/iam/auth/register`（**无鉴权，全系统仅首次可用**，网关已配置 `public = true`）：

- 请求体同创建用户：`username` / `password`（≥ 8 位）/ `nickname` 必填；`email`、`phone`、`avatar` 选填。
- 成功后该用户归属**默认租户**（id=1）并自动获得 `tenant_admin` 角色（绑定全部已启用权限）；前端随后跳登录页走正常登录流程。
- 已有用户后再调返回 `403 registration closed`——已完成初始化的系统不应再暴露初始化页入口。

> ⚠️ 初始化入口先到先得，部署后应立即完成首个管理员注册。完整流程见 [onboarding 03 章「首次初始化管理员」](../onboarding/03-第一次跑通.md)。

---

## 2. Access Token 管理

### 2.1 Token 结构

```json
{
  "header": {
    "alg": "RS256",
    "typ": "JWT",
    "kid": "1"
  },
  "payload": {
    "sub": "u-001",
    "tid": "t-001",
    "username": "alice",
    "nickname": "Alice",
    "roles": ["admin"],
    "permissions": ["iam:user:read", ...],
    "exp": 1720000000,
    "iat": 1719996400,
    "iss": "token-service"
  }
}
```

### 2.2 Token 过期时间

| Token 类型 | 默认 TTL | 说明 |
|------------|----------|------|
| access_token | 3600 秒（1小时） | 用于 API 请求 |
| refresh_token | 604800 秒（7天） | 用于续期 access_token |

### 2.3 主动刷新时机

```javascript
// 推荐：过期前 60 秒主动刷新
const REFRESH_BUFFER = 60; // 秒

function shouldRefresh(expiresAt) {
  const now = Math.floor(Date.now() / 1000);
  return expiresAt - now < REFRESH_BUFFER;
}

// 或者：捕获 401 后刷新
async function requestWithRefresh(url, options) {
  const response = await fetch(url, options);
  if (response.status === 401) {
    await refreshToken();
    return fetch(url, options); // 重试
  }
  return response;
}
```

---

## 3. Refresh Token 流程

### 3.1 刷新 API

```bash
POST /v1/iam/auth/refresh
Content-Type: application/json

{
  "refresh_token": "<refresh_token>"
}
```

**响应**：
```json
{
  "access_token": "eyJhbGc...",
  "refresh_token": "new_refresh_token...",
  "expires_in": 3600
}
```

> ⚠️ **注意**：refresh_token 刷新后会轮换（旧的失效，新的生效）。前端需要更新存储中的 refresh_token。

### 3.2 刷新失败处理

```javascript
async function handleRefreshFailure(error) {
  if (error.status === 401 || error.status === 400) {
    // refresh_token 无效或过期
    clearAuthState();
    redirectToLogin();
  }
}
```

### 3.3 并发刷新防护

```javascript
let refreshPromise = null;

async function safeRefresh() {
  if (refreshPromise) {
    return refreshPromise; // 等待正在进行的刷新
  }
  refreshPromise = doRefresh();
  try {
    return await refreshPromise;
  } finally {
    refreshPromise = null;
  }
}
```

---

## 4. Axios 拦截器实现（推荐）

### 4.1 安装拦截器

```typescript
// src/utils/auth.ts
import axios from 'axios';
import { useAuthStore } from '@/stores/auth';

export function setupAuthInterceptor() {
  const authStore = useAuthStore();

  // 请求拦截器：附加 token
  axios.interceptors.request.use(async (config) => {
    if (authStore.isAuthenticated) {
      // 检查是否需要刷新
      if (authStore.shouldRefresh) {
        await safeRefresh();
      }
      config.headers.Authorization = `Bearer ${authStore.accessToken}`;
    }
    return config;
  });

  // 响应拦截器：处理 401
  axios.interceptors.response.use(
    (response) => response,
    async (error) => {
      const originalRequest = error.config;
      
      if (error.response?.status === 401 && !originalRequest._retry) {
        originalRequest._retry = true;
        
        try {
          await safeRefresh();
          originalRequest.headers.Authorization = `Bearer ${authStore.accessToken}`;
          return axios(originalRequest);
        } catch (refreshError) {
          authStore.logout();
          window.location.href = '/login';
          return Promise.reject(refreshError);
        }
      }
      
      return Promise.reject(error);
    }
  );
}
```

### 4.2 Pinia Store

```typescript
// src/stores/auth.ts
import { defineStore } from 'pinia';
import { ref, computed } from 'vue';
import axios from 'axios';

const REFRESH_BUFFER = 60; // 过期前 60 秒刷新

export const useAuthStore = defineStore('auth', () => {
  const accessToken = ref<string | null>(null);
  const refreshToken = ref<string | null>(null);
  const expiresAt = ref<number | null>(null);
  const user = ref<User | null>(null);

  const isAuthenticated = computed(() => {
    return !!accessToken.value && !!expiresAt.value && Date.now() < expiresAt.value * 1000;
  });

  const shouldRefresh = computed(() => {
    if (!expiresAt.value) return false;
    const now = Math.floor(Date.now() / 1000);
    return expiresAt.value - now < REFRESH_BUFFER;
  });

  async function login(username: string, password: string) {
    const response = await axios.post('/v1/iam/auth/login', { username, password });
    const data = response.data;
    
    accessToken.value = data.access_token;
    refreshToken.value = data.refresh_token;
    expiresAt.value = Math.floor(Date.now() / 1000) + data.expires_in;
    user.value = data.user;
  }

  async function refresh() {
    const response = await axios.post('/v1/iam/auth/refresh', {
      refresh_token: refreshToken.value,
    });
    const data = response.data;
    
    accessToken.value = data.access_token;
    refreshToken.value = data.refresh_token;
    expiresAt.value = Math.floor(Date.now() / 1000) + data.expires_in;
  }

  function logout() {
    accessToken.value = null;
    refreshToken.value = null;
    expiresAt.value = null;
    user.value = null;
  }

  return { accessToken, refreshToken, expiresAt, user, isAuthenticated, shouldRefresh, login, refresh, logout };
});
```

---

## 5. 会话管理

### 5.1 会话列表

```bash
GET /v1/iam/sessions
Authorization: Bearer <token>
```

**响应**：
```json
{
  "sessions": [
    {
      "session_id": "s-001",
      "device": "Chrome on Windows",
      "ip": "192.168.1.100",
      "last_active": "2026-07-13T10:30:00Z",
      "created_at": "2026-07-01T08:00:00Z"
    }
  ]
}
```

### 5.2 登出指定会话

```bash
DELETE /v1/iam/sessions/{session_id}
Authorization: Bearer <token>
```

### 5.3 登出所有会话

```bash
DELETE /v1/iam/sessions
Authorization: Bearer <token>
```

---

## 6. WebSocket 认证

### 6.1 通知服务 WebSocket

通知服务支持 WebSocket 实时推送：

```javascript
// 建立 WS 连接
const ws = new WebSocket('ws://localhost:8083/v1/notification/ws');

// 认证（在连接打开后立即发送）
ws.onopen = () => {
  ws.send(JSON.stringify({
    type: 'auth',
    token: authStore.accessToken,
  }));
};

// 接收消息
ws.onmessage = (event) => {
  const data = JSON.parse(event.data);
  if (data.type === 'announcement') {
    showNotification(data.payload);
  }
};
```

> ⚠️ **注意**：WebSocket 认证由 api-gateway 在 upgrade 时处理，token 放在 URL query parameter 或在 open 后立即发送。

---

## 7. 安全考虑

### 7.1 XSS 防护

- access_token 不要存储在 localStorage（XSS 可读取）
- 使用 httpOnly Cookie（需要后端支持）
- 定期检查页面是否有恶意脚本注入

### 7.2 CSRF 防护

- api-gateway 检查 `Origin` / `Referer` header
- 敏感操作使用 POST/PUT/DELETE

### 7.3 Token 泄露处理

如果发现 token 泄露：
1. 立即调用 `DELETE /v1/iam/sessions/{session_id}` 吊销
2. 或 `DELETE /v1/iam/sessions` 吊销所有会话
3. 修改密码

---

## 8. 多 Tab 同步

### 8.1 场景

用户在多个 Tab 登录，一个 Tab 登出后，其他 Tab 应该也跳转登录页。

### 8.2 实现方式

```javascript
// 登出时广播
localStorage.setItem('logout', Date.now());

// 监听变化
window.addEventListener('storage', (event) => {
  if (event.key === 'logout') {
    authStore.logout();
    redirectToLogin();
  }
});

// 定期检查 token 有效性
setInterval(async () => {
  if (!authStore.isAuthenticated) {
    redirectToLogin();
  }
}, 60000); // 每分钟检查一次
```

---

## 9. 常见问题

### 9.1 Token 过期但还在操作

```javascript
// 拦截器会自动刷新并重试
// 如果刷新失败，弹出登录页
```

### 9.2 刷新 token 时用户正在操作

```
用户正在填写表单，token 过期 → 静默刷新 → 重试原请求 → 成功
```

### 9.3 网络断开导致刷新失败

```javascript
// 检查网络状态
window.addEventListener('offline', () => {
  showToast('网络断开，请检查网络连接');
});

window.addEventListener('online', () => {
  // 网络恢复，尝试刷新
  safeRefresh();
});
```

---

## 10. 关联

- `docs-site/guides/java-integration.md` — 后端 auth 流程
- `services/iam-service/src/routes/auth.rs` — 登录/刷新 API 实现
- `libs/jwt-ctx/src/principal.rs` — JWT 结构定义

---

> **最后更新**：2026-07-26
> **待前端负责人确认**：
> - 实际使用的 token 存储方案
> - 拦截器具体实现
> - 多 Tab 同步实现细节

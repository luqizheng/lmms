# 路由规范

统一前端、api-gateway、后端服务的路由约定。三层保持一致的 URL 结构。

---

## 总则

所有对外 HTTP 接口遵循以下 URL 模板：

```
/api/v1/<service>/<resource>/...
```

- `/api/v1` 全局版本前缀，固定不变。
- `<service>` 取自服务名清单：`iam` / `token` / `audit` / `notification` / `config` / `file` / `api-gateway`。
- `<resource>` 资源名，复数、kebab-case，如 `users`、`operation-logs`、`oauth-clients`。
- 网关与后端之间用 `<service>` 段对齐，后端路由基址统一为 `/<service>`。

---

## 一、前端

### 1.1 环境变量

| 变量 | 开发 | 生产 |
|---|---|---|
| `VITE_API_BASE` | `/api` | `/api` |
| `VITE_PUBLIC_PATH` | `/` | `/` |
| `VITE_ROUTER_HISTORY` | `hash` | `h5` |

`VITE_API_BASE` 必须等于 `/api`，与网关入口一致。

### 1.2 axios 调用路径

所有 `http.request` 第二参数统一为：

```ts
http.request("get", "/api/v1/<service>/<resource>", { params })
```

例：

```ts
http.request("get",  "/api/v1/iam/users", { params })
http.request("post", "/api/v1/token/oauth-clients", { data })
http.request("get",  "/api/v1/audit/operation-logs", { params })
```

禁止直接写 `/iam/...` / `/audit/...` / `/v1/...` 等裸路径。

### 1.3 前端页面路由（vue-router）

- 路由文件位于 `frontend/src/router/modules/*.ts`，由 `index.ts` 自动 glob 导入。
- 路径以 `/` 开头，kebab-case，命名 PascalCase。
- 路由 `rank` 取自 `frontend/src/router/enums.ts`，home 必须为 0。
- `meta.title` 走 `$t("menus.xxx")` 国际化键。
- 鉴权失败落点：`/login`、`/access-denied`、`/server-error`（均在 `remaining.ts`）。
- 不参与菜单的路由通过 `meta.showLink = false` 控制。
- 动态路由接口：`GET /api/v1/iam/auth/menu`（由后端返回 `RouteConfigsTable`）。

### 1.4 开发代理（vite）

```ts
server.proxy["/api"] = {
  target: "http://127.0.0.1:3000",
  changeOrigin: true,
  rewrite: p => p.replace(/^\/api/, "")
}
```

开发态 `/api/*` 直达网关 3000。

### 1.5 生产 nginx（容器内 `frontend/nginx.conf`）

```nginx
location /api/ {
    rewrite ^/api/(.*)$ /$1 break;
    proxy_pass http://api-gateway:3000;
}
```

SPA fallback：

```nginx
location / { try_files $uri $uri/ /index.html; }
```

---

## 二、api-gateway

### 2.1 配置文件位置

`docker/config/api-gateway/routes/<feature>.router.toml`

- 每文件一个服务或一个特性域。
- 文件名与 `<service>` 同名：`iam.router.toml` / `token.router.toml` / `audit.router.toml` / `notification.router.toml` / `config.router.toml` / `file.router.toml`。
- 杂项路由放 `prue_backend.router.toml` 等专用文件。

### 2.2 路由条目模板

```toml
[[route]]
upstream   = "/v1/<service>/<resource>/{rest}"
downstream = "/<service>/<resource>/{rest}"
service    = "<service>-service"
public     = false
```

字段说明：

| 字段 | 必填 | 说明 |
|---|---|---|
| `upstream` | 是 | 网关入口模板（已剥除 `/api`），必须以 `/v1/<service>` 开头 |
| `downstream` | 是 | 转发到微服务的路径模板，可含 query |
| `service` | 是 | Consul 注册名，`<service>-service` |
| `public` | 否 | `true` 跳过 JWT 校验，仅限 `login` / `refresh` / `health` / `/.well-known/**` |
| `ws` | 否 | `true` 升级 WebSocket，仅通知中心使用 |
| `headers` | 否 | 转发时附加请求头，如 `{ "X-Source" = "gateway" }` |

### 2.3 占位符语法

- 占位符以 `{` 与 `}` 包裹变量名，无段限定
- 提取原理：用相邻字面量在请求 url 上锚定切割，把每个 `{var}` 之间的字符串塞进 dict
- 渲染原理：对 downstream 模板做 `{name}` → `dict[name]` 字符串替换
- 变量值可包含 `/`（即可跨段）
- upstream 与 downstream 中的变量名集合必须完全一致
- 同名变量在 upstream 内只允许出现一次
- 两个 `{var}` 之间必须有非空字面量分隔

### 2.4 匹配顺序与唯一性

- 同 `upstream` 不允许重复，启动报 `duplicate upstream`
- 加载期按 upstream 字面量长度降序排序，更具体的路径优先匹配
- 通配路由用 `{rest}` 表达，例如 `/v1/iam/users/{rest}`
- 公共路由（`public = true`）通常以精确条目形式存在

### 2.5 健康检查

每个服务必须暴露 `/<service>/health`：

```toml
[[route]]
upstream   = "/v1/<service>/health"
downstream = "/<service>/health"
service    = "<service>-service"
public     = true
```

### 2.6 WebSocket

仅通知中心一条：

```toml
[[route]]
upstream   = "/v1/notification/ws"
downstream = "/notification/ws"
service    = "notification-service"
ws         = true
```

### 2.7 校验命令

```bash
cargo run -p api-gateway -- validate --routes docker/config/api-gateway/routes
```

改 `routes/*.router.toml` 后必须先 validate 再 commit。

---

## 三、后端配置

### 3.1 服务路由基址

每个 Rust 服务的 axum/actix-web 路由需自带 `/<service>` 前缀（与网关透传后路径一致）：

```rust
// iam-service 示例：routes/auth.rs
pub fn routes(state: AppState) -> Router {
    Router::new()
        .route("/iam/auth/login", post(login))
        .route("/iam/auth/refresh", post(refresh))
        .with_state(state)
}
```

或在外层嵌套（audit-service 模式）：

```rust
// audit-service 示例：routes/operation.rs
pub fn routes(state: AppState) -> Router {
    Router::new()
        .route("/operation", post(create).get(query))
        .with_state(state)
}
// routes/mod.rs
pub fn routes(state: AppState) -> Router {
    Router::new()
        .nest("/audit", operation::routes(state.clone()))
        .nest("/audit", login::routes(state.clone()))
}
```

无论哪种写法，service 暴露的最终路径都必须形如 `/<service>/<resource>`，与网关 `downstream` 完全一致。

服务清单与端口：

| 服务 | 外部端口 | 内部端口 | 前缀 |
|---|---|---|---|
| api-gateway | 3000 | 3000 | — |
| token-service | 6001 | 8080 | `/token` |
| iam-service | 6002 | 8080 | `/iam` |
| notification-service | 6003 | 8080 | `/notification` |
| audit-service | 6004 | 8080 | `/audit` |
| config-service | 6005 | 8080 | `/config` |
| file-service | 6006 | 8080 | `/file` |

### 3.2 资源路径

后端控制器路径以 `<service>/<resource>` 完整形态注册：

```rust
// iam-service 示例：routes/users.rs
.route("/iam/users", get(list_users).post(create_user))
.route("/iam/users/{user_id}", get(get_user).put(update_user).delete(delete_user))
```

资源名复数、kebab-case，例：`users` / `roles` / `operation-logs` / `oauth-clients` / `org-units`。

> 若用 `nest` 聚合（audit 模式），内层路径只需写 `<resource>`，外层 `nest` 写 `/<service>`，最终效果一致。

### 3.3 Swagger / OpenAPI

- 启用开关：`SWAGGER_ENABLED`（默认开发开、生产关）。
- 文档路径：`/swagger-ui/`、`/api-docs/openapi.json`。
- 与前端 `/api-docs/openapi.json` 通过网关 `/.well-known/**` 或专用 `<service>/swagger` 路由暴露。

### 3.4 健康检查

每个服务内置 `/<service>/health`，返回：

```json
{ "status": "ok", "service": "<service>-service", "version": "x.y.z" }
```

注册至 Consul 健康检查路径 `<service>/health`，由 `HEALTH_CHECK_REQUIRED` 强制。

### 3.5 CORS 与代理头

- 网关注入 `X-Forwarded-For` / `X-Forwarded-Proto`，后端按代理头读取真实 IP。
- 跨域由网关统一处理，后端不重复设置 CORS 头。

### 3.6 错误响应

遵循 `contracts/error-codes.yaml`，统一格式：

```json
{ "code": "IAM_USER_NOT_FOUND", "message": "...", "request_id": "uuid" }
```

由 `libs/common-error` 注入中间件，禁止各服务自行序列化。

---

## 四、对照表

| 层 | 路径示例 | 备注 |
|---|---|---|
| 浏览器 | `https://host/api/v1/iam/users` | 前端 axios 发起 |
| nginx / vite | `/api/v1/iam/users` → `api-gateway:3000/v1/iam/users` | 去掉 `/api` |
| api-gateway | `upstream="/v1/iam/users/{rest}"` → `downstream="/iam/users/{rest}"` → `iam-service:8080/iam/users/{rest}` | 占位符提取 + 渲染 |
| 后端 | `iam-service:8080/iam/users` | 路由自带 `/iam` 前缀 |

任何一层偏离该链路即为违规，需同步修复三层。
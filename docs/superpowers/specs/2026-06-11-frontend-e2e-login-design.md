# 端到端登录 — 设计

日期：2026-06-11
状态：待审阅
范围：frontend（Vue 3）+ user-service + token-service + api-gateway
上游设计：
- `docs/superpowers/specs/2026-06-10-user-service-password-login-design.md`（后端登录）
- `docs/superpowers/specs/2026-06-10-auth-login-security-design.md`（安全策略）

## 目标

实现从前端到后端的完整登录链路，账号 `admin/admin123` 可成功登录并跳转到首页：

1. 在 user-service 中实现密码登录 + 首位注册（注册者自动获得 admin 角色）。
2. 前端打通注册 → 登录 → 拿 JWT → 拉用户信息 → 跳转首页。
3. 路径前缀统一为 `/api/v1/`，与现有 v1 网关路由一致。
4. 端到端验证：admin/admin123 实际登录成功。

## 设计决策摘要

| 决策 | 选择 | 理由 |
|------|------|------|
| 后端登录位置 | user-service 直连 | 沿用 `user-service-password-login-plan`；credential-login-service 是 stub |
| 登录路径 | `/api/v1/auth/login` | 与现有 `/api/v1/users/**` 等 v1 路由一致 |
| 注册路径 | `/api/v1/auth/register` | 同上 |
| setup-status 路径 | `/api/v1/auth/setup-status` | 同上 |
| 注册策略 | 仅首位 = admin，后续拒绝 | 用户决策；防垃圾账号 |
| Token 交换 | 沿用 token-service `/api/v1/tokens/access` | 已有 `auth:code` 流程；前端无需改交互 |
| client_secret 加载 | dev 明文 + 启动现算哈希 | 业界 dev 简化做法；prod 走环境变量 |
| 网关路由 | 新建 `auth.router.toml`，删 `credential-login.router.toml` | 替代 stub 路由 |
| 前端 dev 联调 | Vite dev proxy `/api` → `http://localhost:3000` | 决策确认；前后端同源零 CORS |
| 角色预置 | migrations INSERT IGNORE admin/user | 幂等；首启动即可用 |
| API 协议风格 | REST + JSON；前端 axios | 沿用现有 |
| 错误约定 | 4xx `{error: "...", code?: "..."}`；成功 2xx 数据 | 与现有 `routes::auth::login` 一致 |
| 鉴权 | 注册/登录/setup-status public；其余业务需 Bearer JWT | 沿用现有 |
| 日志 | user-service 写 `user_login_log`；注册写 `users` + `user_roles` | 沿用现有 |
| 测试栈 | testcontainers (mysql + redis) + Rust 集成测试 | 沿用现有 |
| 退出登录 | 暂只前端清 token，不调后端 | 范围控制；后续可加 revoke |

## 架构

```
┌──────────┐  POST /api/v1/auth/register (首次)
│ 浏览器    │──────────────────────────────────┐
│  :8848   │  POST /api/v1/auth/login         │
└────┬─────┘  POST /api/v1/tokens/access     │
     │       GET  /api/v1/users/me           │
     │ Vite dev proxy  /api → :3000          │
     ▼                                        ▼
┌──────────┐  路由转发  ┌──────────┐  ┌──────────────┐
│ Frontend │ ─────────►│ api-gw    │─►│ user-service │
└──────────┘  :3000     │  (Rust)  │  │  (Rust,新增) │
                        └─────┬────┘  └──────────────┘
                              │
                              ├─► token-service (Rust) :3002
                              │     - /api/v1/tokens/access 用 auth:code 换 JWT
                              │     - /internal/users/:id/claims 拉角色权限
                              │
                              └─► MySQL / Redis
```

## 数据模型与迁移

新建 `migrations/2026_06_11_auth_login_and_register.sql`：

```sql
USE oa3;

ALTER TABLE users
  ADD COLUMN password_hash VARCHAR(255) NULL AFTER user_code,
  ADD COLUMN last_login_at DATETIME NULL,
  ADD COLUMN last_login_ip VARCHAR(45) NULL;

CREATE TABLE IF NOT EXISTS user_login_log (
  id BIGINT UNSIGNED NOT NULL AUTO_INCREMENT PRIMARY KEY,
  user_id BIGINT UNSIGNED NULL,
  username VARCHAR(64) NOT NULL,
  ip VARCHAR(45) NOT NULL,
  user_agent VARCHAR(255) NOT NULL,
  success TINYINT(1) NOT NULL,
  fail_reason VARCHAR(32) NULL,
  login_at DATETIME(3) NOT NULL,
  created_at DATETIME(3) NOT NULL DEFAULT CURRENT_TIMESTAMP(3),
  INDEX idx_login_log_username (username),
  INDEX idx_login_log_user_id (user_id),
  INDEX idx_login_log_success (success),
  INDEX idx_login_log_login_at (login_at)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;

INSERT IGNORE INTO roles (code, name, status) VALUES
  ('admin', '系统管理员', 1),
  ('user',  '普通用户',   1);
```

幂等：可重跑。`setup_required` 用 `SELECT COUNT(*) FROM users WHERE status=1` 实时计算（无新表）。

## 后端 API

### `GET /api/v1/auth/setup-status`

- 鉴权：public
- 响应：
  - 200 `{setup_required: true}` 当 `count=0`
  - 200 `{setup_required: false}` 当 `count>0`
- 错误：500（DB 不可用）

### `POST /api/v1/auth/register`

- 鉴权：public（仅 setup_required=true 有效）
- 请求体：
  ```json
  {"username":"admin","password":"admin123","email":"x@y.z","phone":"","nickname":""}
  ```
- 校验：
  - username 长度 3-32，正则 `^[a-zA-Z0-9_.-]+$`
  - password 长度 ≥ 6
  - email/phone 可选，email 用简单正则、phone 11 位数字
- 业务：
  1. 事务 `SELECT COUNT(*) FROM users WHERE status=1`；>0 → 403
  2. 查 `roles` 表 `code='admin'` 的 id；找不到 → 500（migration 未跑）
  3. argon2 哈希（沿用 `LoginSecurityCfg`）
  4. 插入 `users`（含 user_code `U-<nanoid(10)>`，status=1）
  5. 插入 `user_roles` 绑定 admin
  6. 返回 201 `{id, username}`
- 错误：
  - 400 字段缺失/格式错
  - 403 `{error:"registration closed"}`
  - 409 `{error:"username taken"}`
  - 500

### `POST /api/v1/auth/login`

- 鉴权：public
- 沿用 `user-service-password-login-plan` 流程
- 路径：handler router 写死 `/api/v1/auth/login`
- 响应：
  - 200 `{code, expires_in: 300}`
  - 400 缺字段
  - 401 `{error:"invalid credentials"}`
  - 423 `{error:"account locked", retry_after}`

## 网关与客户端

- 删 `docker/config/api-gateway/routes/credential-login.router.toml`（指向 stub）
- 新建 `docker/config/api-gateway/routes/auth.router.toml`：
  ```toml
  [[route]]
  match = "/api/v1/auth/**"
  service = "user-service"
  public = true
  rewrite = { type = "strip_prefix", prefix = "/api/v1" }
  ```
- `docker/config/api-gateway/routes/token.router.toml` 已覆盖 `/api/v1/tokens/**` 与 `/.well-known/**`，保持
- `docker/config/api-gateway/routes/user.router.toml` 已覆盖 `/api/v1/users/**`，保持
- `docker/config/token-service/clients.json`：`api-gateway` 客户端的 `client_secret_hash` 启动时由 token-service 用 `dev-gateway-secret` 明文现算 argon2id 哈希后注入内存（dev 简化）。生产用 `CLIENT_SECRET_HASH` 环境变量。

## 前端改动

### `frontend/vite.config.ts`

```ts
server: {
  port: VITE_PORT,
  host: "0.0.0.0",
  proxy: {
    "/api": { target: "http://localhost:3000", changeOrigin: true }
  },
  warmup: { clientFiles: ["./index.html", "./src/{views,components}/*"] }
}
```

### `frontend/src/api/auth.ts`

```ts
import { http } from "@/utils/http";

export interface LoginRequest { username: string; password: string; }
export interface LoginResult  { code: string; expires_in: number; }
export const login = (data: LoginRequest) =>
  http.request<LoginResult>("post", "/api/v1/auth/login", { data });
export const getSetupStatus = () =>
  http.request<{ setup_required: boolean }>("get", "/api/v1/auth/setup-status");

export interface RegisterRequest {
  username: string; password: string;
  email?: string; phone?: string; nickname?: string;
}
export interface RegisterResult { id: number; username: string; }
export const register = (data: RegisterRequest) =>
  http.request<RegisterResult>("post", "/api/v1/auth/register", { data });

export const logout = () => http.request<void>("post", "/api/v1/auth/logout");
```

### `frontend/src/api/token.ts`

全部路径 `/tokens/*` → `/api/v1/tokens/*`。

### `frontend/src/api/user.ts`

全部路径 `/users/*` → `/api/v1/users/*`。

### `frontend/src/utils/http/index.ts`

```ts
const whiteList = ["/api/v1/auth/", "/api/v1/tokens/access", "/api/v1/tokens/refresh"];
```

### `frontend/src/views/login/index.vue`

- `onMounted`：`getSetupStatus()`，若 `setup_required` 跳 `/register`
- 模板底部链接：`<router-link v-if="setupRequired" to="/register">首次部署？注册管理员</router-link>`
- `onLogin` 流程不变

### 新建 `frontend/src/views/login/register.vue`

字段：username / password / confirm / email（可选）/ 提交
- 校验两次密码一致
- 提交 → 201 → 调 `loginByUsername` → 跳 `/welcome`
- 失败 403 → 提示"注册已关闭"

### `frontend/src/router/modules/account.ts`

加 `/register` 路由：

```ts
{ path: "/register", component: () => import("@/views/login/register.vue"),
  meta: { title: "注册管理员", showLink: false, hidden: true } }
```

### `frontend/src/router/index.ts`

`whiteList = ["/login"]` 改为 `["/login", "/register"]`。

### `frontend/.env.development`

```
VITE_CLIENT_SECRET = dev-gateway-secret
```

`store/modules/user.ts` 已读 `VITE_CLIENT_SECRET`，无需改。

## 模块布局（user-service 新增）

```
services/user-service/src/
├── routes/
│   └── auth.rs            # 扩 router() 暴露 /api/v1/auth/{login,register,setup-status}
├── service/
│   └── auth_svc.rs        # 扩 register() / setup_status()
├── models/
│   └── login_log.rs       # 不变
└── dto.rs                 # 扩 RegisterReq / RegisterResp
```

## 测试

### 单元/集成测试（user-service）

沿用 `user-service-password-login-plan` Task 10，再加：
- `register_first_user_becomes_admin`
- `register_second_user_rejected`
- `register_invalid_password`
- `setup_status_reflects_user_count`

### 单元/集成测试（token-service）

- `client_secret_plaintext_hashed_on_startup`：用 `dev-gateway-secret` 调 access 应成功
- `invalid_client_secret_rejected`：错误 secret → 401

### 端到端冒烟（手动）

1. 启动 mysql / redis / consul
2. 跑迁移 `migrations/2026_06_11_auth_login_and_register.sql`
3. `cargo run -p user-service` & `cargo run -p token-service` & `cargo run -p api-gateway`
4. `cargo run -p api-gateway -- validate --routes docker\config\api-gateway\routes`
5. `cd frontend && pnpm dev`
6. 浏览器 `http://localhost:8848/#/login` → 跳 `/register`
7. 填 `admin / admin123` → 跳 `/welcome`
8. 退出 → `admin / admin123` → 跳 `/welcome`
9. 故意输错 6 次 → 423 + retry_after

## 不做的事

- 不做邮箱/短信验证
- 不做密码重置
- 不做图形验证码
- 不做 OAuth2 第三方
- 不删 `services/credential-login-service/`（独立清理 plan）
- 不改 token-service 现有 JWT 签发逻辑
- 不动 user-service 现有 CRUD 路由
- 不动 `api-gateway` 现有除 credential-login 外的路由

## 文件改动清单

新建：
- `migrations/2026_06_11_auth_login_and_register.sql`
- `services/user-service/src/service/auth_svc.rs`（扩）
- `services/user-service/src/routes/auth.rs`（扩）
- `services/user-service/tests/auth_register_test.rs`
- `services/token-service/tests/client_secret_startup_test.rs`
- `docker/config/api-gateway/routes/auth.router.toml`
- `frontend/src/views/login/register.vue`
- `docs/superpowers/specs/2026-06-11-frontend-e2e-login-design.md`

修改：
- `services/user-service/Cargo.toml`（+argon2, +nanoid）
- `services/user-service/src/dto.rs`（+RegisterReq/Resp）
- `services/user-service/src/main.rs`（挂新路由）
- `services/user-service/config/config.yaml`（+login_security）
- `services/user-service/docker/config/user-service/config.yaml`（同步）
- `services/token-service/src/store/client.rs`（启动时明文 → argon2 哈希）
- `docker/config/api-gateway/routes/`（删 credential-login.router.toml）
- `frontend/vite.config.ts`（+proxy）
- `frontend/src/api/auth.ts`（路径 + register/setup-status）
- `frontend/src/api/token.ts`（路径）
- `frontend/src/api/user.ts`（路径）
- `frontend/src/utils/http/index.ts`（白名单）
- `frontend/src/views/login/index.vue`（setup 检测 + 链接）
- `frontend/src/router/modules/account.ts`（+register）
- `frontend/src/router/index.ts`（白名单）
- `frontend/.env.development`（+VITE_CLIENT_SECRET）

## 风险与注意

- **client_secret 内存哈希**：仅 dev；prod 走环境变量并禁用明文加载。
- **首位注册窗口**：事务内二次校验防并发导致两位 admin。
- **user_code 生成**：`U-<nanoid(10)>`，碰撞概率极低；冲突时 SQL 唯一键兜底。
- **gateway `strip_prefix` `/api/v1`**：handler 内部路径 `auth/login`（不再带 v1）。
- **Vite dev proxy**：仅 dev；prod nginx 反代需单独配（不在本设计）。
- **CORS**：dev 走代理同源无 CORS；prod 由 nginx 配。
- **沙箱限制**：所有 cargo 命令在真实 PowerShell（vcvarsall x64）跑。

## 验收

- 全新环境跑迁移后，前端能完成"注册 admin/admin123 → 跳首页 → 退出 → 登录 admin/admin123 → 跳首页"
- `cargo test -p user-service -- --test-threads=1` 全过
- `cargo test -p token-service -- --test-threads=1` 全过
- `cargo run -p api-gateway -- validate --routes docker\config\api-gateway\routes` 0 error

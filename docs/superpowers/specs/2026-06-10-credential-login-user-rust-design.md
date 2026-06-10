# Credential-Login & User Service 重头做（Rust）设计

日期：2026-06-10
状态：待审阅
取代：`2026-06-10-go-to-rust-migration-design.md`（旧 spec 基于"1:1 复刻 Go 契约"，本设计推翻该方向）

---

## 1. 目标

删除现有 Go 版 `auth-service`、`user-service`，新建 Rust 版 `credential-login-service` 与 `user-service`；保留并改造 `token-server` 为 `token-service`，承担所有 token 签发（user + service）。不沿用现有 HTTP API 契约，不重发 .NET / Java SDK。

| 决策 | 选择 | 理由 |
|---|---|---|
| 服务拆分 | credential-login + user 独立两服务 | 单一职责；v2 多登录方式可平移 |
| 凭证登录命名 | `credential-login-service` | 强调"凭证登录"语义，v2 仍可扩展 |
| 登录方式 v1 | 仅密码 | YAGNI；v2 微信/OTP/短信是后续 plan |
| token 签发 | token-service 负责 user JWT + service JWT | 单一签发源；密钥集中 |
| 用户表结构 | 沿用 `docs/member_org.sql` 七张表 + `UserCode` 字段 | 与现有业务库 oa3 对齐 |
| UserCode 规则 | 8 位 NanoID（大小写敏感） | 短、可读、不可枚举为登录凭证 |
| 数据库名 | `oa3` | 沿用 |
| API 版本 | `/api/v1/...` | 留 v2 升级空间 |
| 兼容旧 SDK | 否 | 用户明确"不需要理会兼容" |

---

## 2. 服务清单

| 服务 | 路径 | 状态 | 职责 |
|---|---|---|---|
| `services/credential-login-service` | 新建 Rust + Axum | 新建 | 账密验证、内部 code 兑换、刷新、改密 |
| `services/token-service` | `services/token-server` 改造 | 改名 + 改造 | user JWT + service JWT 签发、JWKS、黑名单 |
| `services/user-service` | 新建 Rust + Axum | 新建 | 用户/角色/组织/岗位 CRUD + claim 维护 + 内部端点 |
| `services/api-gateway` | 沿用 | 不变 | JWT 验签 + 路由 |
| `services/notification-service` | 沿用 plan | 沿用 | 通知（与本设计无关） |
| `services/auth-service`（Go） | 删除 | 删 | — |
| `services/user-service`（Go） | 删除 | 删 | — |

---

## 3. 数据库

### 3.1 `oa3`（沿用 member_org.sql）

| 表 | 字段变化 |
|---|---|
| `member_org` | 不变 |
| `member_post` | 不变 |
| `member_role` | 不变 |
| `member_role_claim` | 不变 |
| `member_user` | **+ `UserCode VARCHAR(16) NOT NULL UNIQUE`** |
| `member_user_claim` | 不变 |
| `member_user_role` | 不变 |

迁移脚本：
```sql
-- migrations/2026_06_10_add_user_code.sql
ALTER TABLE oa3.member_user
  ADD COLUMN UserCode VARCHAR(16) NOT NULL UNIQUE AFTER Id;
-- 已存在数据：用 8 位 NanoID 回填
UPDATE oa3.member_user
  SET UserCode = (
    SELECT CONCAT(
      ELT(FLOOR(RAND()*26)+1, 'A','B','C','D','E','F','G','H','I','J','K','L','M','N','O','P','Q','R','S','T','U','V','W','X','Y','Z'),
      ELT(FLOOR(RAND()*26)+1, 'a','b','c','d','e','f','g','h','i','j','k','l','m','n','o','p','q','r','s','t','u','v','w','x','y','z'),
      ELT(FLOOR(RAND()*10)+1, '0','1','2','3','4','5','6','7','8','9'),
      ELT(FLOOR(RAND()*10)+1, '0','1','2','3','4','5','6','7','8','9'),
      ELT(FLOOR(RAND()*10)+1, '0','1','2','3','4','5','6','7','8','9'),
      ELT(FLOOR(RAND()*10)+1, '0','1','2','3','4','5','6','7','8','9'),
      ELT(FLOOR(RAND()*10)+1, '0','1','2','3','4','5','6','7','8','9'),
      ELT(FLOOR(RAND()*10)+1, '0','1','2','3','4','5','6','7','8','9')
    )
  )
  WHERE UserCode = '' OR UserCode IS NULL;
```

> **回填示例仅供说明**，实施时使用 Rust 应用层生成并 UPDATE（保证唯一性）。

### 3.2 `token`（沿用 token-server）

不变。

### 3.3 `login`（新建，credential-login-service 自用）

```sql
CREATE TABLE login_audit (
  Id           BIGINT UNSIGNED NOT NULL AUTO_INCREMENT,
  UserId       VARCHAR(36)     NULL,        -- 用户 UUID；账号不存在时为 NULL
  Username     VARCHAR(64)     NOT NULL,    -- 实际提交的用户名
  Ip           VARCHAR(45)     NOT NULL,
  UserAgent    VARCHAR(255)    NULL,
  Success      TINYINT(1)      NOT NULL,
  FailReason   VARCHAR(32)     NULL,        -- bad_password | user_not_found
  CreatedAt    DATETIME        NOT NULL,
  PRIMARY KEY (Id),
  KEY idx_user (UserId, CreatedAt),
  KEY idx_username (Username, CreatedAt)
) ENGINE=InnoDB;
```

---

## 4. 端点

### 4.1 credential-login-service

| Method | Path | 说明 |
|---|---|---|
| POST | `/api/v1/auth/login` | 账密 → 内部 `code`（5min Redis TTL） |
| POST | `/api/v1/auth/exchange-code` | `code` → access + refresh JWT |
| POST | `/api/v1/auth/refresh` | refresh → 新 access |
| POST | `/api/v1/auth/logout` | 撤销（黑名单） |
| GET | `/api/v1/auth/me` | 当前用户信息（凭 JWT） |
| POST | `/api/v1/auth/change-password` | 改密 |

### 4.2 user-service

**业务端点（挂 api-gateway）**：

| Method | Path | 说明 |
|---|---|---|
| GET/POST | `/api/v1/users` | 列表 / 创建 |
| GET/PUT/DELETE | `/api/v1/users/{id}` | 详情 / 改 / 删 |
| GET/POST | `/api/v1/roles` | 列表 / 创建 |
| GET/PUT/DELETE | `/api/v1/roles/{id}` | 详情 / 改 / 删 |
| GET/POST | `/api/v1/orgs` | 列表（树形）/ 创建根 |
| GET/PUT/DELETE | `/api/v1/orgs/{id}` | 详情 / 改 / 删 |
| GET/POST | `/api/v1/posts` | 列表 / 创建 |
| GET/PUT/DELETE | `/api/v1/posts/{id}` | 详情 / 改 / 删 |
| POST/DELETE | `/api/v1/users/{id}/roles` | 绑/解角色 |
| POST/DELETE | `/api/v1/roles/{id}/claims` | 角色绑/解 claim |
| POST/DELETE | `/api/v1/users/{id}/claims` | 用户绑/解 claim |

**内部端点（仅内网，不挂 api-gateway）**：

| Method | Path | 说明 |
|---|---|---|
| GET | `/internal/v1/users/by-username/{username}` | 取 PasswordHash + Id（credential-login-service 验证用） |
| GET | `/internal/v1/users/{id}/claims` | 聚合 user + role claims + dept → claims JSON（token-service 签 user JWT 用） |
| GET | `/internal/v1/users/by-user-code/{userCode}` | 按 userCode 查详情（业务微服务引用） |
| POST | `/internal/v1/users` | 由 credential-login-service / token-service 同步创建 |

### 4.3 token-service（沿用 + 新增）

**沿用**：access / refresh / api_key / resource / revoke / `/.well-known/jwks.json`

**新增**：
- `POST /api/v1/tokens/user-jwt`：service 调（凭 service token），body 含 userId、claims JSON，签发 user JWT

### 4.4 api-gateway（沿用）

- 验签密钥仍指向 `secrets/public.pem`（token-service 持有的 RSA 私钥签发，公钥验签）
- 路由表追加 credential-login-service / user-service / token-service（新名字）的路由

---

## 5. 协作流

### 5.1 登录（账密 → JWT）

```
client → api-gateway
  POST /api/v1/auth/login {username, password}
   │
   ▼
credential-login-service
  1. 调 user-service GET /internal/v1/users/by-username/{u}
     → 拿 PasswordHash + UserId
  2. Argon2 verify(password, PasswordHash)
  3. 写 login_audit
  4. 生成内部 code (随机 UUID)
  5. Redis SET code:<code> = <UserId> EX 300
  6. 返回 {code, expires_in: 300}

client → api-gateway
  POST /api/v1/auth/exchange-code {code}
   │
   ▼
credential-login-service
  1. Redis GET code:<code> → UserId
  2. DEL code:<code>
  3. 调 user-service GET /internal/v1/users/{id}/claims
     → 拿 roles / permissions / dept_id
  4. 调 token-service POST /api/v1/tokens/user-jwt
     headers: Authorization: Bearer <service_token>
     body: {user_id, claims: {...}}
     → access_token + refresh_token
  5. 返回 {access_token, refresh_token, expires_in}
```

### 5.2 后续请求

```
client
  Authorization: Bearer <access_token>
   │
   ▼
api-gateway
  - RSA 验签 + Redis 黑名单
  - 注入 X-User-Id / X-User-Code / X-User-Roles / X-User-Perms / X-User-Dept
   │
   ▼
user-service / notification-service / 其它
  - 从 X-User-* 头解析身份（不验签 JWT）
  - 业务级授权：每个 handler 按 role / permission 自行判定
```

### 5.3 服务间调用

```
credential-login-service
  Authorization: Bearer <service_token>     ← 启动时从 token-service 拿
  X-Service-Caller: credential-login-service
   │
   ▼
user-service / token-service
  - 内部端点
  - 不挂 api-gateway
  - 监听内网端口
```

---

## 6. JWT claims

**user JWT**（token-service 签发）：

| claim | 类型 | 来源 | 说明 |
|---|---|---|---|
| `sub` | string | token-service | user UUID（member_user.Id） |
| `iss` | string | token-service | `"token-service"` |
| `aud` | string | token-service | `"coder-micro-services"` |
| `exp` | int64 | token-service | 过期时间 |
| `iat` | int64 | token-service | 签发时间 |
| `jti` | string | token-service | UUID |
| `client_id` | string | token-service | `"credential-login-service"` |
| `scope` | string | token-service | `"access"` |
| `user_code` | string | user-service | 8 位 NanoID |
| `user_name` | string | user-service | UserName |
| `dept_id` | int | user-service | 部门 ID（可空） |
| `roles` | []string | user-service | 角色名列表 |
| `permissions` | []string | user-service | 细粒度权限点列表 |

**service JWT**（token-service 签发，client_credentials 流）：

| claim | 类型 | 说明 |
|---|---|---|
| `sub` | string | 客户端 ID（如 `"credential-login-service"`） |
| `iss` | string | `"token-service"` |
| `aud` | string | 目标服务名 |
| `exp` | int64 | |
| `iat` | int64 | |
| `jti` | string | UUID |
| `scope` | string | 空格分隔 |
| `token_type` | string | 固定 `"service"` |

---

## 7. Cargo workspace 调整

```toml
[workspace]
members = [
  "libs/common",
  "libs/jwt-ctx",
  "libs/authz",
  "services/token-service",       # 原 token-server 改名
  "services/api-gateway",
  "services/credential-login-service",  # 新建
  "services/user-service",               # 新建
  "services/notification-service",
]
resolver = "2"
```

**`libs/jwt-ctx` 改造**：从"解析 X-User-* 头"扩展为"从 X-User-* 头构造 Principal + service 端发 HTTP 时自动注入 X-User-* 头"。`Principal` 字段：
- `user_id: Uuid`（注意：sub 是 UUID 字符串）
- `user_code: String`（8 位 NanoID）
- `dept_id: Option<i64>`
- `roles: Vec<String>`
- `permissions: HashSet<String>`

**`libs/authz` 改造**：`require_role("user.admin")` 沿用；新增 `require_permission("user.write")`。

---

## 8. 内部端点鉴权

user-service 的 `/internal/v1/*` 与 token-service 的 `/api/v1/tokens/user-jwt` 必须**验签** service JWT：
- 读 `Authorization: Bearer ...`
- 验 RSA 签名 + `token_type == "service"` + `aud` 匹配
- 不挂 api-gateway，监听内网端口

**实施**：在 user-service 与 token-service 内部用 `libs/authz` 提供 `require_service_token("user-service")` 中间件；该中间件验签而非仅 decode。

---

## 9. 删除 / 改造清单

| 路径 | 处理 |
|---|---|
| `services/token-server/` | 改名 `services/token-service/`，Cargo.toml package.name 同步；新增 `/api/v1/tokens/user-jwt` |
| `services/auth-service/` | 整目录删除（Go + Docker） |
| `services/user-service/`（Go） | 整目录删除 |
| `services/credential-login-service/` | 新建 |
| `services/user-service/`（Rust 新） | 新建 |
| `libs/go-jwt-ctx/`、`libs/go-authz/` | 删除 |
| `scripts/e2e_test.go` | 删除 |
| 根 `go.mod`、`go.sum`、`go.work` | 删除 |
| `.gitlab-ci.yml` | 删 go_test；rust_test 改全 workspace；新增 build_credential_login_service、build_user_service（Rust） |
| `scripts/test-orchestrator.ps1` | 同步服务名（`token-service`、`credential-login-service`、`user-service`） |
| `scripts/lib/Test-Runner.ps1` | Go test 分支删除 |
| `.trae/rules/project_rules.md` | 删除 Go 规则，加 Rust 规则 |
| `infrastructure/docker-compose.*.yaml` | image 改 Rust |
| `Dockerfile.{auth,user}-service` | 改 Rust builder |
| `Dockerfile.token-service`（原 token-server） | 改名 + Rust builder |
| `docs/superpowers/specs/2026-06-10-go-to-rust-migration-design.md` | 标记 **Superseded by this spec** |
| `docs/superpowers/plans/2026-06-10-go-to-rust-migration-plan.md` | 同上 |

---

## 10. 风险与缓解

| 风险 | 缓解 |
|---|---|
| 旧 SDK（.NET / Java）无法调用新 API | 用户接受；后续按需重发 |
| 数据库 oa3 已有数据 | member_user 加 `UserCode` 列，Rust 应用层回填保证唯一 |
| `PasswordHash` 字段格式（Asp.Net Identity V2/V3） | v1 完全重写为 Argon2 PHC 格式；旧 hash 一次性转换或强制重置 |
| token-server → token-service 改名影响 CI/CD 与文档 | 一次性原子切换；保留旧镜像 24h |
| 内部端点被公网访问 | 部署规范：内网监听 + 防火墙；首次部署前 smoke |
| service JWT 缓存 | credential-login-service 进程内 LRU（不依赖 Redis），过期前 60s 余量 |

---

## 11. 实施顺序

1. **DB 迁移**：加 `UserCode` 列 + 回填（独立 PR，DDL 优先）
2. **token-server 改名 token-service** + 加 `/api/v1/tokens/user-jwt` 端点（沿用现有 RSA 密钥对）
3. **libs/jwt-ctx 改造**：增加 `user_code` 字段
4. **libs/authz 改造**：增加 `require_service_token` 中间件
5. **user-service Rust 新建**（含 `/internal/v1/*` 内部端点）
6. **credential-login-service Rust 新建**（含 service JWT 缓存 + Argon2 verify + 调 user-service / token-service）
7. **api-gateway 路由更新**（加 credential-login-service / user-service / token-service 路由）
8. **删除 Go 资产** + 旧 auth-service 镜像清理
9. **CI / 脚本 / 规则同步**
10. **收尾**：`cargo fmt --all`、`cargo clippy --workspace --all-targets -- -D warnings`、`cargo test --workspace --all-targets`

---

## 12. 测试策略

| 层 | 工具 | 覆盖 |
|---|---|---|
| 单元 | `cargo test` | libs 全部；handler 业务逻辑 |
| 集成 | `cargo test --test integration_test` | 端到端（依赖 MySQL/Redis 容器） |
| E2E | curl + Redis 手工 smoke | 登录流、code 兑换、刷新、注销 |

**契约来源**：本设计是 v1 全新，无旧契约约束。集成测试基于本 spec 直接写。

---

## 13. YAGNI

- 不做 2FA / 邮箱验证 / 短信验证
- 不做防爆破 / 锁定（仅 login_audit 记录）
- 不做字段级脱敏
- 不重发 .NET / Java SDK
- v2 微信 / OTP / 短信登录：各自独立微服务，**后续 plan**
- service JWT 仅 credential-login-service ↔ token-service ↔ user-service 三方用，不上 OAuth 全套
- 不引入新 Rust 框架（沿用 axum 0.8 + sqlx + redis）

---

## 14. 改动清单汇总

| 路径 | 改动 |
|---|---|
| `services/credential-login-service/` (新建) | 全部代码、配置、Dockerfile |
| `services/user-service/` (新建) | 全部代码、配置、Dockerfile |
| `services/token-server/` → `services/token-service/` | 改名；加 user-jwt 端点 |
| `Cargo.toml` | workspace members 调整 |
| `libs/jwt-ctx/src/principal.rs` | 加 `user_code` 字段 |
| `libs/authz/src/` | 加 `require_service_token` |
| `migrations/2026_06_10_add_user_code.sql` (新建) | DDL |
| `infrastructure/docker-compose.*.yaml` | image 调整 |
| `Dockerfile.{credential-login,user,token}-service` | Rust builder |
| `docker/config/api-gateway/routes/{credential-login,user,token}.router.toml` (新建) | 路由 |
| `docker/config/{credential-login,user,token}-service/config.yaml` (新建) | 配置 |
| `.gitlab-ci.yml` | 删 go_test；改 rust_test；改 build jobs |
| `scripts/test-orchestrator.ps1` | service list 同步 |
| `scripts/lib/*.ps1` | 移除 Go 分支 |
| `.trae/rules/project_rules.md` | 删 Go 规则 |
| 删除：`services/auth-service/`、`libs/go-jwt-ctx/`、`libs/go-authz/`、`scripts/e2e_test.go`、根 `go.mod`/`go.sum`/`go.work` | — |
| `docs/superpowers/specs/2026-06-10-go-to-rust-migration-design.md` | 标 Superseded |
| `docs/superpowers/plans/2026-06-10-go-to-rust-migration-plan.md` | 标 Superseded |

---

## 15. 自审

- **占位符**：无 TBD / TODO
- **内部一致性**：服务清单 / 数据库 / 端点 / 协作流 / 改动清单 互相对应
- **范围**：单次 spec 可执行；实施顺序拆为 10 步，每步可在一次 plan 周期完成
- **歧义**：userCode 生成在 Rust 应用层（保证唯一）；service JWT 仅三方用，约束明确
- **取代关系**：明确标记旧 spec / plan 失效

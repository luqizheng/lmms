# Go 服务整体迁 Rust 设计 [SUPERSEDED]

> **状态：已废弃（Superseded）**。已被 [`2026-06-10-credential-login-user-rust-design.md`](file:///d:/projects/coder-micro-services/docs/superpowers/specs/2026-06-10-credential-login-user-rust-design.md) 取代。**本文件不再实施**。仅作历史记录保留。

---

# Go 服务整体迁 Rust 设计 [ORIGINAL, DO NOT EXECUTE]

日期：2026-06-10
状态：待审阅（已废弃）
关联：`2026-06-10-microservice-jwt-verification-design.md`、`2026-06-10-auth-login-security-design.md`、`2026-06-10-notification-service-plan.md`

---

## 1. 目标

将仓库内现存的所有 Go 服务与共享库一次性切换为 Rust 实现，**不再使用 Go**。同步删除 Go 资产并清理 CI / 本地脚本 / AI 规则中的 Go 残留。

| 决策 | 选择 | 理由 |
|---|---|---|
| 迁移策略 | Big-Bang 一次性切换 | 团队决策，避免长期双栈维护 |
| API 契约 | 完全保持现有 HTTP 契约 | .NET / Java SDK 不需重发 |
| MySQL schema | 复用现有表结构 | 零数据迁移 |
| Go 代码去向 | 切换完成后删除 | 与 Big-Bang 一致 |
| Cargo 布局 | services 子目录不动，libs 拆 3 个 crate | 与现有 api-gateway / token-server 路径一致，团队认知负担小 |

---

## 2. 现状盘点

| 资产 | 类型 | 路径 |
|---|---|---|
| api-gateway | Rust + Axum | `services/api-gateway/` |
| token-server | Rust + Axum | `services/token-server/` |
| auth-service | Go + Gin | `services/auth-service/` |
| user-service | Go + Gin | `services/user-service/` |
| notification-service | 计划新增 Rust | `services/notification-service/`（plan 已写） |
| libs/digua_common | Rust | `libs/digua_common/` |
| libs/go-jwt-ctx | Go | `libs/go-jwt-ctx/` |
| libs/go-digua_authz | Go | `libs/go-digua_authz/` |
| scripts/e2e_test.go | Go | `scripts/e2e_test.go` |
| .gitlab-ci.yml | CI | 含 `go_test` + `rust_test` job |
| scripts/test-orchestrator.ps1 | 本地编排 | `Get-ServiceList` 含 Go 服务 |
| .trae/rules/project_rules.md | AI 规则 | 含 Go 路径匹配 |
| sdk/java + sdk/dotnet | SDK | 调 HTTP API，不动 |

---

## 3. 目标态

### 3.1 Cargo workspace

```toml
[workspace]
members = [
  "libs/digua_common",
  "libs/jwt-ctx",
  "libs/digua_authz",
  "services/token-server",
  "services/api-gateway",
  "services/auth-service",
  "services/user-service",
  "services/notification-service",
]
resolver = "2"
```

### 3.2 服务清单

| 服务 | 语言 | 路径 | 端点前缀 |
|---|---|---|---|
| api-gateway | Rust | `services/api-gateway/` | `/api/*` 路由 |
| token-server | Rust | `services/token-server/` | `/api/tokens/*` |
| auth-service | Rust | `services/auth-service/` | `/api/auth/*` |
| user-service | Rust | `services/user-service/` | `/api/users/*`、`/api/roles/*`、`/api/permissions/*` |
| notification-service | Rust | `services/notification-service/` | `/api/notifications/*` |

### 3.3 共享 crate

| crate | 取代 | 职责 |
|---|---|---|
| `libs/digua_common` | 现有 + 扩展 | 错误、配置、Argon2、MySQL/Redis 池、HTTP client |
| `libs/jwt-ctx` | `libs/go-jwt-ctx` | `Principal` extractor，解析 `X-User-*` 头 |
| `libs/digua_authz` | `libs/go-digua_authz` | `require_role("...")` 中间件 |

### 3.4 Go 资产目标数量

- 源文件（`*.go`）：0
- `go.mod` / `go.sum`：0
- CI 中 Go job：0
- 脚本中 Go test 路径：0

---

## 4. 共享 crate 设计

### 4.1 libs/digua_common（扩展现有）

**现有**：`error`、`config`、`token`（arg 结构）。

**新增**：

| 模块 | 职责 | 依赖 |
|---|---|---|
| `password` | Argon2 hash 与 verify | `argon2` |
| `db` | `MySqlPool` 工厂 + 健康检查 | `sqlx` |
| `redisx` | `ConnectionManager` 工厂 | `redis` |
| `http` | 内部 `reqwest::Client` 单例（service-to-service） | `reqwest` |
| `error::AppError` | 统一 `IntoResponse`，与现有风格一致 | `axum` + `thiserror` |

### 4.2 libs/jwt-ctx（替换 libs/go-jwt-ctx）

```rust
pub struct Principal {
    pub user_id: u64,
    pub dept_id: Option<u64>,
    pub dept_leader: bool,
    pub roles: Vec<String>,
    pub permissions: HashSet<String>,
}

#[async_trait]
impl<S: Send + Sync> FromRequestParts<S> for Principal {
    type Rejection = AppError;
    async fn from_request_parts(parts: &mut Parts, _: &S) -> Result<Self, Self::Rejection> {
        // 从 X-User-Id / X-User-Dept / X-User-Dept-Leader /
        // X-User-Roles / X-User-Perms 头解析
    }
}
```

**与 Go 版差异**：字段统一为 `Principal`，方法 `has_role(&str)` / `has_permission(&str)`。

### 4.3 libs/digua_authz（替换 libs/go-digua_authz）

```rust
pub fn require_role<S: Clone + Send + Sync + 'static>(role: &'static str)
  -> impl Fn(S, Request, Next) -> ... + Clone
```

- 中间件读取 `Principal`，检查 `roles` 是否包含 `role`
- 缺失返回 `403 forbidden`
- 提供 `RequireRoles(&[&'static str])` 复合变体

---

## 5. 服务 Rust 化方案

### 5.1 services/auth-service

**端点 1:1 复刻**：

| Method | Path | 说明 |
|---|---|---|
| POST | `/api/auth/login` | 账密 → token-server 换 code |
| POST | `/api/auth/refresh` | refresh token → 新 access |
| POST | `/api/auth/logout` | 撤销 token |
| POST | `/api/auth/register` | admin only |
| GET | `/api/auth/me` | 当前用户信息 |
| POST | `/api/auth/login-log` | 登录审计（已有 login-security 改造） |

**关键依赖**：
- `argon2 0.5`（替换 bcrypt）
- `sqlx 0.8`（mysql，runtime-tokio-rustls）
- `redis 0.27`
- `reqwest 0.12` 调 token-server / user-service
- `libs/jwt-ctx`、`libs/digua_authz`、`libs/digua_common`

**配置**：`config/config.yaml` 与 Go 版字段一致（保留 `login_security` 段）。

**DDL 复用**：`usr_user`、`usr_login_log` 等表零改动。

### 5.2 services/user-service

**端点 1:1 复刻**：

| Method | Path | 说明 |
|---|---|---|
| GET/POST/PUT/DELETE | `/api/users[/{id}]` | 用户 CRUD |
| GET/POST/PUT/DELETE | `/api/roles[/{id}]` | 角色 CRUD |
| GET/POST/PUT/DELETE | `/api/permissions[/{id}]` | 权限 CRUD |
| POST/DELETE | `/api/roles/{id}/permissions` | 角色授权 |
| POST/DELETE | `/api/users/{id}/roles` | 用户绑角色 |
| GET | `/internal/users/{id}/claims` | 内部端点，登录时聚合 claims |

**关键依赖**：`sqlx`、`libs/jwt-ctx`、`libs/digua_authz`、`libs/digua_common`。

**DDL 复用**：`usr_user`、`usr_role`、`usr_permission`、`usr_user_role`、`usr_role_permission` 零改动。

### 5.3 services/notification-service

按 `2026-06-10-notification-service-plan.md` 实施，并入 workspace。

### 5.4 services/api-gateway 与 services/token-server

**保持现有 Rust 实现**。仅做：
- 校验 X-User-* 头字段（与新 `Principal` 解析一致）
- `services/token-server/src/issuer.rs` 接受 `extra_claims: serde_json::Value`（`microservice-jwt-verification` 设计已定）

---

## 6. 契约保护

**当前 Go 服务含集成测试**（`services/auth-service/integration_test.go`、`services/user-service/integration_test.go`）。这些测试同时也是**契约 fixture**：

- 实现阶段：把 Go 测试中所有 HTTP 请求/响应序列抽出为 Rust 集成测试 fixture
- 切换前：以现有 SDK（Java + .NET）做 E2E smoke
- 切换后：Rust 端单测对照 fixture，HTTP 状态码、JSON schema、header 全部一致

**新增 `tests/specs/auth-service/*.yaml` 与 `tests/specs/user-service/*.yaml`**，用现有 `coder-spec-to-code` skill 跑契约验证。

---

## 7. 删除清单

| 路径 | 处理 |
|---|---|
| `libs/go-jwt-ctx/` | 整目录删除 |
| `libs/go-digua_authz/` | 整目录删除 |
| `services/auth-service/handlers/*.go` | 删除（保留目录） |
| `services/auth-service/models/*.go` | 删除（保留目录） |
| `services/auth-service/store/*.go` | 删除（保留目录） |
| `services/auth-service/config/*.go` | 删除（保留目录） |
| `services/auth-service/main.go`、`routes.go`、`consul.go` | 删除 |
| `services/auth-service/go.mod`、`go.sum` | 删除 |
| `services/auth-service/integration_test.go` | 删除（契约为新 spec） |
| `services/user-service/**` | 同上 |
| `scripts/e2e_test.go` | 删除 |
| `scripts/go.mod`、`scripts/go.sum` | 删除 |
| `go.work`（如存在） | 删除 |

**保留**：`services/{auth,user}-service/Dockerfile` 文件名 + 镜像名，仅替换 builder 为 `rust:1.78-slim`。

---

## 8. CI / 脚本 / 规则调整

### 8.1 .gitlab-ci.yml

- 删除 `go_test` job
- `rust_test` 改为 `cargo test --workspace --all-targets --verbose`
- `build_auth_service` / `build_user_service`：替换 Dockerfile 内容为 Rust builder，镜像名保留

### 8.2 scripts/

- `test-orchestrator.ps1`：`Get-ServiceList` 移除 `auth-service`（Go）和 `user-service`（Go）；新增 Rust 版同名服务
- `lib/Test-Runner.ps1`：移除 `go test` 分支，新增 `cargo test --workspace` 调用
- `lib/Spec-Validator.ps1`：spec 路径无须改（`tests/specs/**/yaml` 已含新服务）
- `lib/Docker-Helper.ps1`：service 列表同步

### 8.3 .trae/rules/project_rules.md

- 删除 `services/*/**.go` 路径触发表
- 删除 `**go test**` 失败 → skill 触发表
- 提交前缀规则：移除 `feat(<svc>)` 中的 Go 注释
- 增加 `services/*/src/**.rs` → `coder-run-tests -Service <name>`（已有，更新名字）

### 8.4 infrastructure/

- `docker-compose.yml`：4 个服务的 image 改为新 Rust 镜像
- `Dockerfile.{auth,user}-service` 替换为 Rust 多阶段 builder

---

## 9. 风险与缓解

| 风险 | 缓解 |
|---|---|
| 契约漂移（API 行为变化） | Go 测试抽 fixture；Rust 集成测试对照；切换前 E2E smoke |
| Big-Bang 切换瞬间故障 | 低峰期停服窗口；端点 + DB schema 不变，回滚仅需切镜像 |
| 共享 crate API 设计错 | 现有 `libs/digua_common` 已是 Rust 风格，沿用；jwt-ctx / digua_authz 按现有 Go 包的对外行为 1:1 |
| Rust 编译时长 | 共享 crate 用 `path` 依赖走 workspace cache |
| Go 残留（go.mod / .gitlab-ci.yml） | 删除清单 + CI 校验：仓库顶层 grep `*.go` 与 `go.mod` 应为空 |
| 旧 Go 镜像回滚 | 旧 Docker image 保留 24h 备查，不重新打 tag |

---

## 10. 实施顺序

1. **workspace 拓建**：新增 3 个 libs crate（jwt-ctx / digua_authz）stub，Cargo members 加入
2. **libs/jwt-ctx 实现**：extractor + 单测
3. **libs/digua_authz 实现**：require_role + 单测
4. **libs/digua_common 扩展**：Argon2 + MySQL/Redis 池 + reqwest client
5. **services/auth-service Rust 化**：骨架 → config → db → redis → handlers → routes → 集成测试
6. **services/user-service Rust 化**：同上节奏
7. **services/notification-service 接入 workspace**：按既有 plan
8. **删除 Go 资产**：执行 §7 清单
9. **CI / 脚本 / 规则同步**：执行 §8 清单
10. **收尾**：`cargo fmt --all`、`cargo clippy --workspace --all-targets -- -D warnings`、`cargo test --workspace --all-targets` 全绿

---

## 11. 测试策略

| 层 | 工具 | 覆盖 |
|---|---|---|
| 单元 | `cargo test` | libs 全量；handler 业务逻辑 |
| 集成 | `cargo test --test integration_test` | 端到端（依赖 MySQL/Redis 容器） |
| 契约 | `coder-spec-to-code` 跑 `tests/specs/{auth,user,notification}-service/*.yaml` | HTTP 端点契约 |
| E2E | `sdk/java` + `sdk/dotnet` 跑 mvn verify / dotnet test | SDK 不破坏 |

**契约 fixture 来源**：
- 抽 `services/auth-service/integration_test.go` 中所有 `httptest` 调用为 YAML
- 同上 user-service

---

## 12. YAGNI

- 不重写 .NET / Java SDK
- 不重新设计 API 契约
- 不重设计 MySQL schema
- 不引入新中间件
- 不引入新 Rust 框架（沿用 Axum 0.8）
- 不重设计 RBAC 模型
- 不实现 service-token（按 `2026-06-10-microservice-jwt-verification` 设计待真实需求再做）

---

## 13. 改动清单汇总

| 路径 | 改动 |
|---|---|
| `Cargo.toml` | workspace members 扩到 8 项 |
| `libs/jwt-ctx/` (新建) | Principal extractor + 单测 |
| `libs/digua_authz/` (新建) | require_role + 单测 |
| `libs/digua_common/src/{password,db,redisx,http}.rs` (新建) | 共享基础设施 |
| `services/auth-service/` (重写) | 删 Go、写 Rust |
| `services/user-service/` (重写) | 删 Go、写 Rust |
| `services/notification-service/` (新建) | 按既有 plan |
| `services/{auth,user}-service/Dockerfile` | 改 Rust builder |
| `infrastructure/docker-compose.*.yaml` | image 改新镜像 |
| `.gitlab-ci.yml` | 删 go_test；rust_test 改 workspace |
| `scripts/test-orchestrator.ps1` | service list 同步 |
| `scripts/lib/*.ps1` | 移除 Go test 分支 |
| `.trae/rules/project_rules.md` | 删 Go 规则，加 Rust 规则 |
| `tests/specs/auth-service/*.yaml` (新建) | 契约 |
| `tests/specs/user-service/*.yaml` (新建) | 契约 |
| `libs/go-jwt-ctx/`、`libs/go-digua_authz/` | 删除 |
| `scripts/e2e_test.go` | 删除 |
| 根 `go.mod`、`go.sum`、`go.work`（如有） | 删除 |

---

## 14. 自审

- 占位符：无 TBD / TODO
- 内部一致性：workspace members 与 §3.2 服务清单、§3.3 crate 列表、§13 改动清单一致
- 范围：单次 spec 可执行；实施顺序拆为 10 步，每步可在一次 plan 周期内完成
- 歧义：API 契约 1:1 复刻的判定以现有 Go integration_test.go 为准，§6 与 §11 已固化来源

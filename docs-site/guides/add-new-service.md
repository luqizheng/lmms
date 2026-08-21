# 如何在本体系中新增一个微服务

> 端到端实战手册：从「我想加个新服务」到「CI 全绿、可上线」。  
> 配套 skill：`.agents/skills/coder-new-service/`（AI 用的脚手架细节）。本文档面向**人 + AI 协作**，给流程、给决策、给坑，不重复模板。

## 0. 何时该新建一个服务（决策树）

```
Q1. 这个能力是新的业务限界上下文吗？
├─ 否 / 只是 iam 的新表/字段  →  加 route（.agents/skills/coder-add-route）
│                                + 加 migration（.agents/skills/coder-migrate-db）
├─ 是
│   Q2. 它需要独立的部署节奏、扩缩容、SLA、独立团队？
│   ├─ 否 / 跟现有服务强耦合   →  并入现有服务（加路由 + 模块）
│   └─ 是
│       Q3. 它有独立的存储？还是必须复用 MySQL `superpowers` 共库？
│       ├─ 共库                  →  走「Rust 微服务 + 共享 MySQL」标准路径（本文 §3 起）
│       └─ 完全独立库 / 异构存储  →  走「Rust 微服务 + 独立 DB」，建新 DATABASE_URL（仍是本文路径，但加 §4.5）
```

判断原则：**服务数量是稀缺资源**。每加一个服务都要喂 8 项规范 + 6 个 CI 门禁 + 路由注册 + 文档同步。新加一个的门槛应该比「在现有服务里加个模块」高。

---

## 1. 取名字（一次性敲定，所有下游配置都用这一组）

| 用途 | 命名 | 示例 |
|------|------|------|
| Rust crate / Cargo workspace member | `kebab-case` | `notification-service` |
| Consul service name | 同 crate 名 | `notification-service` |
| Docker image / container | 同 crate 名 | `notification-service` |
| 容器端口（业务进程监听） | `8080`（统一约定） | `SERVER_PORT=8080` |
| **网关对外路径前缀** | `kebab-case`，单数 | `/v1/notification/...` |
| 业务表前缀（共享 MySQL） | `<3-4 字母>_`，**全小写蛇形** | `ntf_`（取 4 字母）、`iam_` |
| 错误码前缀（`contracts/error-codes.yaml`） | 大写 3 字母 | `NTF`、`IAM`、`CFG` |
| 权限码前缀（`contracts/permissions.yaml`） | 大写 3 字母 + `_` | `NTF_ANNOUNCEMENT_CREATE` |
| 事件路由前缀 | 小写服务名 + `.` | `notification.announcement.published` |

**强约束**：
- DB 表前缀必须在 `contracts/db-conventions.md` §「表名前缀映射」表格中**先新增**一行才能用，CI 才会放过；
- 错误码 / 权限码必须先在 YAML 里加，**改完跑 `node scripts/contracts-gen.js`** 让 `libs/digua_common-error/src/codes.rs` 与 `libs/digua_authz/src/perms.rs` 重新生成。**禁止手改这两个生成文件**（参见 `ai-context.md` §7 坑点 9）；
- 同一个服务**只能有一个**表前缀——不允许同服务里出现两个前缀。

---

## 2. 前期准备清单（动手前必须明确）

| 项 | 必须的答案 |
|---|------|
| 服务名 | `xxx-service`（决定全部下游配置） |
| 一句话业务定位 | 写进 README 第一段与 `AGENTS.md` 标题 |
| 数据库 | `MySQL 共享 superpowers 库` / `SQLite 单机` / `MySQL 独立库` / `无` |
| 表前缀 | `xxx_`（必须先在 `contracts/db-conventions.md` 加登记） |
| 是否生产事件 | 是 → 写 `xxx_outbox_events` 表 + 新增 outbox publisher bin；否 → 跳过 §4.6 |
| 是否消费事件 | 是 → 在 `contracts/events.yaml` 登记 + 队列声明；否 → 跳过 |
| 是否调用其他服务 | 是 → 列出调用清单（决定 service-jwt 申请 + §4.7）；否 → 默认 |
| 是否需要 WebSocket | 是 → 参照 notification-service `/notification/ws` 模式；否 → 跳过 |
| 端口 | 容器内固定 `8080`，对外映射在 `infrastructure/docker-compose*.yml` 里 |
| 是否启 OTel | 是 → `digua_common-telemetry` + `digua_common-telemetry-axum`；否 → 跳过 |
| Swagger 默认开关 | dev 默认 `true`，prod 默认 `false`（按 iam / notification / config 的惯例） |

---

## 3. 脚手架（让 AI 干活）

直接调 `.agents/skills/coder-new-service/`：

```
触发词：「新增一个 xxx-service」/「scaffold xxx-service」/「create xxx service」
```

skill 会产出：

- `services/<name>/{Cargo.toml, Dockerfile, AGENTS.md, README.md}`
- `services/<name>/src/{main, lib, config, error, routes/mod, openapi}.rs`
- 视 DB 类型加 `migrations/{mysql,sqlite}/V001__init.sql` + `db.rs`
- 视路由需求加 `src/routes/<feature>.rs` + `tests/<feature>_test.rs` + `src/bin/dump-openapi.rs`

skill 完成后**人工核对**（skill 是模板，错的可能性极低但不是零）：

1. `Cargo.toml` 顶部的 `package.name` 与目录名一致；
2. `src/openapi.rs` 的 `info(title=...)` 改成服务名（不要带 `Service` 后缀的 title 显得怪）；
3. `AGENTS.md` 里把模板中的 `<service-name>` 全部替换；
4. 把服务加进根 `Cargo.toml` `[workspace] members` 数组（**漏了 `cargo check` 找不到 crate**，参见 skill 「Common Mistakes」第 1 条）。

---

## 4. 业务接入（脚手架后必做的 6 件事）

### 4.1 JWT 身份解析（必做）

每个 handler 用 `libs/jwt-ctx::Principal` extractor 拿身份，**禁止读 `X-User-*` headers**。

```rust
use digua_jwt_ctx::Principal;

async fn create_announcement(
    me: Principal,
    State(state): State<AppState>,
) -> AppResult<...> {
    if !me.has_role("admin") && !me.has_permission("NTF_ANNOUNCEMENT_CREATE") {
        return Err(AppError::Forbidden);
    }
    let actor_id = me.user_id;   // 用于审计 actor 字段
    // ...
}
```

详见 `.trae/rules/auth-rules.md`。

### 4.2 多租户隔离（若涉及业务数据，必做）

按现有约定，所有业务表**带 `tenant_id BIGINT UNSIGNED NOT NULL`** 字段。所有 handler 加 `TenantScope` extractor，所有 service / querybuilder 都注入 `scope`。

```rust
pub async fn list(&self, scope: &TenantScope, qb: &mut QueryBuilder) -> AppResult<Vec<X>> {
    qb.and_tenant(scope);
    // 自动追加 `WHERE tenant_id = ?`
}
```

点查询路径（get / update / delete）也走 `where_id(scope, id)`，否则会被 `check-service-standard.js` 拒绝（参见 `ai-context.md` §7 坑点 18）。最低限度：handler 接 `Principal` + `TenantScope`，service 第一个参数接 `&TenantScope`，每条 SQL `WHERE` 必须含 `tenant_id = ?`。

### 4.3 表前缀（必做，且**先于**第一版迁移）

1. 在 `contracts/db-conventions.md` §「表名前缀映射」表格新增一行：服务名 + 前缀 + 示例表；
2. 写 `V001__init.sql` 时所有 `CREATE TABLE` 必须以该前缀起头（如 `ntf_announcements`）；
3. CI 门禁 `node scripts/check-db-naming.js` 会扫 migration 与 `sqlx` 字符串，违规直接红（参见 `ai-context.md` §7 坑点 18）。

### 4.3.1 用户引用字段类型（必做，新表 / 新列）

**所有引用用户身份的列统一 `VARCHAR(64)`**——本平台 user id 是随机字符串（UUID / Nano ID），不是自增整数。适用：`user_id` / `created_by` / `updated_by` / `operator_id` / `actor_id` / `owner_id`。

- ❌ 错误：`created_by BIGINT NOT NULL` —— JWT `sub` 是随机字符串，写不进去；
- ✅ 正确：`created_by VARCHAR(64) NOT NULL`（SQLite 写 `TEXT`）；
- 现存历史表长度不一（`VARCHAR(32/36/64/128)`），**不动旧表**，新表一律 64。

权威定义见 `contracts/db-conventions.md` §「用户引用字段类型约定」。

### 4.4 权限码 / 错误码注册（必做，先于实现）

```yaml
# contracts/permissions.yaml 加：
NTF_ANNOUNCEMENT_CREATE:
  service: notification-service
  description: 创建公告
NTF_ANNOUNCEMENT_PUBLISH:
  service: notification-service
  description: 发布公告
```

```yaml
# contracts/error-codes.yaml 加：
NTF_ANNOUNCEMENT_NOT_FOUND:
  http: 404
  message: "公告不存在"
```

```bash
node scripts/contracts-gen.js   # 重新生成 libs/digua_common-error/src/codes.rs 与 libs/digua_authz/src/perms.rs
git diff libs/                   # 确认生成产物已更新
```

### 4.5 数据库迁移（必做，forward-only）

调 `.agents/skills/coder-migrate-db/`。要点：

- `V{N}__<topic>.sql` 命名，`N` 严格递增，**不要编辑已发布迁移**（参见 `ai-context.md` §7 坑点 5）；
- 共享 MySQL 时 schema 同时支持 MySQL 与（若适用）SQLite，CI 会同时扫；
- 类型在两版间不同（如 SQLite 没有 `DATETIME(3)`，用 `TEXT` + `strftime`），双版本独立写。

### 4.6 事件生产 / 消费（按需）

**生产事件**（同步发送跨服务副作用）：

1. `migrations/mysql/V{N+1}__xxx_outbox_events.sql` 建 outbox 表（命名 `<prefix>_outbox_events`）；
2. 业务代码在 **同一 sqlx 事务**里写业务行 + 调用 `digua_common_event::MySqlOutboxRepo::enqueue(&mut tx, &Envelope::new("<svc>", payload))`；
3. 新增 `services/<svc>-outbox-publisher/`（参考 `iam-outbox-publisher/` 模板），它是一个独立 bin（同镜像不同 CMD）；
4. 在 `infrastructure/config/rabbitmq/definitions.json` 的 events exchange + queue 拓扑里登记新队列；
5. `services/<svc>/Cargo.toml` 加 `[[bin]]` 出 publisher binary。

**消费事件**：

1. 在 `services/<svc>/src/bin/` 加 consumer bin（参考 `notification-service/src/bin/` 的写法）；
2. **必须按 `event_id` 幂等**（at-least-once 保证，参见 `ai-context.md` §7 坑点 19）；
3. payload 用结构化类型，不要用 `serde_json::Value`（Value 仅用于消费侧反序列化）。

### 4.7 调用其他服务 / 对外内部 API（按需）

如果本服务要调其他服务的内部能力：

- **面向网关的 API**：直接走 `http://api-gateway:3000/v1/<svc>/...`，带 JWT；
- **服务间直连**（仅在性能敏感或网关不通的场景）：用 service-jwt（`aud: ["service", "<self>"]` + `tid` claim），走 `127.0.0.1:<port>`；
- 如果本服务要**被**其他服务调用，且需要「按角色 / 按部门查 user_id」之类的能力，参照 iam 的 `/iam/internal/users/by-role/:role_id` + `/iam/internal/users/by-dept/:dept_id` 在自己服务里加 `/<svc>/internal/*` 端点 + `service-jwt` extractor。

---

## 5. 路由暴露（API Gateway 接入）

新建 `infrastructure/config/api-gateway/routes/<svc>.router.toml`：

```toml
[[route]]
match    = "/v1/<svc>/**"
service  = "<svc>-service"        # Consul 注册名
public   = false                  # 默认要 JWT
# rewrite = [...]                  # 按需，如要 strip `/api`
```

**校验**（任何改动都要跑，CI 也会跑）：

```bash
cargo run -p api-gateway -- validate --routes infrastructure/config/api-gateway/routes
```

完整路由规则（match 模式、rewrite 链、热重载）见 [`api-gateway-routing.md`](api-gateway-routing.md)。

公开路由（无需 JWT，如 `/health`、`/iam/auth/login`）加 `public = true`，但要明白意味着无身份校验——一般只给健康检查和登录用。

---

## 6. 服务发现与编排

### 6.1 Consul 注册（registrator 项目级）

业务进程**不直接调用 Consul**。`infrastructure/docker-compose-infra.yml` 中的 `gliderlabs/registrator` 容器挂 `/var/run/docker.sock`，从业务容器的 **Docker labels** 自动注册 / 注销到 Consul catalog。要点：

- 给业务 service 加 `labels: *svc-reg-labels-<svc>` anchor（详见 [`service-registration.md`](service-registration.md) §2.2）；
- labels 里的 `SERVICE_PORT` **必须是容器内端口**（不是 host 端口）；
- `SERVICE_<PORT>_CHECK_HTTP` 的端口数字必须与 `SERVICE_PORT` 一致（例：`SERVICE_PORT=8081` ↔ `SERVICE_8081_CHECK_HTTP=/health`）；
- 业务 `main.rs` 不需要任何 Consul 注册调用——Phase 5-A-2 已下线 `libs/consul-registrar`。

### 6.2 docker-compose 编排

把服务加进 `infrastructure/docker-compose-services.yml`（**所有业务服务共用一个文件**，不要新开 yaml）：

```yaml
  <svc>-service:
    build:
      context: ..
      dockerfile: services/<svc>/Dockerfile
    ports:
      - "<host_port>:8080"          # 容器内固定 8080
    environment:
      SERVER_HOST: "0.0.0.0"
      SERVER_ADDR: "0.0.0.0:8080"
      DATABASE_URL: "mysql://root:rootpass@mysql:3306/superpowers"
      RUST_LOG: "info"
      OTEL_EXPORTER_OTLP_ENDPOINT: "http://otel-collector:4317"  # 可选
      CONSUL_ADDR: "http://consul:8500"
    depends_on:
      mysql:
        condition: service_healthy
      consul:
        condition: service_healthy
    networks:
      - superpowers
```

**注意**：

- 端口对照（按 `infrastructure/docker-compose-services.yml` + `service-registration.md` §2.3）：容器内端口是服务进程实际监听的端口（8081/8082/8083/8084/8087/8085:8081/8086:8082 等）；host 端口为宿主机开发联调用。现有服务 host 端口：gateway `3000`、token `8082`、iam `9081`（mesh 残留，待统一 8081）、notification `8083`、prue `8087`、audit `8084`、config `8085`、file `8086`。新服务挑一个**未被占用**的 host port（建议 8xxx 段，避开 6000-6999 / 3000 / 8080-8087 / 9081 / 3001 / 8500 / 9090 等已被基础设施或可观测性占用的端口）；
- 若需要独立卷（持久存储），加 `volumes:`；若不需要就别加；
- 若同时有 `services/<svc>-outbox-publisher/` 这种配套 bin，**同一个镜像不同 CMD**，在同一文件里加第二个 service entry（参考 `iam-service` + `iam-outbox-publisher` 的写法）。

---

## 7. 可观测性（按需，但强烈推荐）

### 7.1 Prometheus metrics

`coder-new-service` 模板默认挂 `/metrics` 端点（用 `prometheus::TextEncoder`），CI 已要求。无需额外动作。

### 7.2 OTel tracing（仅 HTTP 服务）

要加入 tracing 链路（gateway → your-service），需三步：

1. `Cargo.toml` 加依赖：
   ```toml
   digua_common-telemetry = { path = "../../libs/digua_common-telemetry", features = ["otlp"] }
   digua_common-telemetry-axum = { path = "../../libs/digua_common-telemetry-axum", features = ["otlp"] }
   ```
2. `src/main.rs` 在最前面 `init_tracing("<svc>")`；
3. `src/routes/mod.rs` 在 `Router::new()` 后接 `.layer(digua_common_telemetry_axum::tracing_layer())`；
4. docker-compose 里设 `OTEL_EXPORTER_OTLP_ENDPOINT=http://otel-collector:4317`。

详见 [`otel-tracing.md`](otel-tracing.md)。**两个 feature flag 缺一个 = 静默 stdout-only**（参见 `ai-context.md` §7 坑点 11）。

### 7.3 健康检查

`/health` 必须能在 1s 内返回 200，**不要在 health handler 里查 DB**（启动期 DB 还没 ready 会导致 Consul 一直 critical + 网关永远拿不到实例）。DB 连接失败留给 `/metrics` 与日志暴露。

---

## 8. 测试

### 8.1 单测

`tests/` 目录下放 smoke / 集成测试。本地**不要跑** `cargo test`（缺 `cc` 链接器，参见 `ai-context.md` §7 坑点 2），统一走 docker：

```bash
node scripts/run-tests-docker.js <svc-name>
```

### 8.2 Testcontainers 集成测试

参考 `config-service` / `iam-service` 的写法：`dev-dependencies` 加 `testcontainers` + `testcontainers-modules::mysql`，在测试里 `MySqlContainer::new(...)`，启动后跑 `sqlx::migrate!` 应用本服务的迁移。优点：CI 跑无需宿主 MySQL。

### 8.3 E2E

新服务的关键路径加 e2e 用例到 `tests/e2e/api/cases/<svc>.yaml`（yaml 驱动）。要加 smoke 用例加 `smoke: true`，便于开发期快速迭代（参见 `ai-context.md` §7 坑点 16）。

---

## 9. CI 门禁（push 前**必须全过**）

```bash
# 1. 服务规范
node scripts/check-service-standard.js
# 期望：<svc> 8/8 通过

# 2. OpenAPI / 错误码 / 权限码（若改了 contracts 或 services）
node scripts/check-spec.js

# 3. 契约一致性
node scripts/contracts-gen.js --check

# 4. 表名前缀
node scripts/check-db-naming.js

# 5. OpenAPI yaml 漂移
node scripts/dump-openapi-specs.js
git diff --exit-code -- 'services/*/openapi.yaml'

# 6. Rust 测试（docker）
node scripts/run-tests-docker.js <svc-name>
```

**全绿才能 push**。GitLab CI 在 `test` 阶段会再跑一遍（参见 `ai-context.md` §9）。本地可选装 pre-commit 自动拦截：

```bash
pre-commit install   # 装 .pre-commit-config.yaml
# 或回退：
cp scripts/git-hooks/pre-commit .git/hooks/pre-commit && chmod +x .git/hooks/pre-commit
```

---

## 10. 文档同步（容易被漏的 4 处）

| 文件 | 改什么 |
|------|------|
| `docs-site/ai-context.md` §2 服务地图 | 加新服务一行（端口、DB、Tech、Role、OTel） |
| `docs-site/ai-context.md` §10 导航地图 | 加「要新建服务 → 本指南」一行 |
| `services/<svc>/AGENTS.md` | 8 项规范打勾，端点域列全 |
| `services/<svc>/README.md` | Quick-start + 端点 + env 表 |
| `docs-site/decisions/ADR-NNN-*.md` | **可选**：仅当引入新的架构选择（如决定加 Redis）才写 |

---

## 11. 上线检查清单（端到端）

按顺序勾，全过才算完事：

- [ ] `Cargo.toml` workspace members 含新服务
- [ ] `services/<svc>/AGENTS.md` 8 项规范全部勾选 + 端点域列全 + env 表完整
- [ ] `services/<svc>/README.md` 一句话定位 + quick-start + 端点表
- [ ] 所有业务表带服务前缀，迁移在 `migrations/mysql/V001__init.sql`（或 sqlite 双版本）
- [ ] 所有用户引用字段（`user_id` / `created_by` / `updated_by` / `operator_id` / `actor_id` / `owner_id`）为 `VARCHAR(64)`（SQLite 为 `TEXT`），**不是 `BIGINT`**
- [ ] `contracts/error-codes.yaml` / `contracts/permissions.yaml` 加完且 `contracts-gen` 已重生成
- [ ] `infrastructure/config/api-gateway/routes/<svc>.router.toml` 已加，`api-gateway validate` 通过
- [ ] `infrastructure/docker-compose-services.yml` 加 service entry + `labels: *svc-reg-labels-<svc>` anchor，`SERVICE_PORT` 是容器内端口，host port 未占用
- [ ] 若启用 OTel：`digua_common-telemetry` + `digua_common-telemetry-axum` 都有 `otlp` feature
- [ ] `node scripts/check-service-standard.js` 8/8
- [ ] `node scripts/check-spec.js` exit 0
- [ ] `node scripts/contracts-gen.js --check` exit 0
- [ ] `node scripts/check-db-naming.js` exit 0
- [ ] `node scripts/dump-openapi-specs.js` 之后 `git diff` 干净
- [ ] `node scripts/run-tests-docker.js <svc>` 全绿
- [ ] `docs-site/ai-context.md` §2 §10 已更新
- [ ] 至少一个 e2e 用例（smoke 子集即可）
- [ ] 一个 commit，message 如 `feat(<scope>): scaffold xxx-service`

---

## 12. 常见坑点（踩过的真问题）

1. **忘了加 workspace member** — `cargo check` 找不到 crate。症状：`error[E0433]: failed to resolve: use of undeclared crate`。修：根 `Cargo.toml` 加进去。
2. **`utoipa::OpenApi::to_yaml()` 编译失败** — `Cargo.toml` 的 `utoipa` 必须带 `yaml` feature（参见 `ai-context.md` §7 坑点 1）。
3. **本地 `cargo build` 链接器报错** — 缺 `cc`。统一跑 `node scripts/run-tests-docker.js <svc>`，或在 `zhcoder-docker-registry.com:8000/builder/rust:1.96-slim-musl` 容器内构建（参见 `ai-context.md` §7 坑点 2）。
4. **服务起来了但 Consul 健康检查一直 critical** — registrator 自动从 `SERVICE_<PORT>_CHECK_HTTP` 创建 HTTP check。检查：① `SERVICE_PORT` 与 `SERVICE_<PORT>_CHECK_HTTP` 后缀端口一致；② handler 在容器内 `curl -sf http://127.0.0.1:<port><path>` 通；③ `/health` 不查 DB（参见 §7.3）；④ `docker inspect <container>` 的 labels 段里有 `SERVICE_*` 全部键。
5. **改完 handler 注解但 swagger 没更新** — `openapi.yaml` 是 **utoipa 自动生成**的，改代码后必须跑 `node scripts/dump-openapi-specs.js`，CI 用 `git diff --exit-code` 守护。**禁止手改 yaml**（参见 `ai-context.md` §7 坑点 4）。
6. **手改 `libs/digua_common-error/src/codes.rs`** — 该文件是 `contracts-gen.js` 生成品，改完下次跑就丢了。改 `contracts/error-codes.yaml` 再重生成（参见 `ai-context.md` §7 坑点 9）。
7. **表名撞名** — 共库下所有表必须带前缀（参见 `contracts/db-conventions.md`）。`iam_users` 与历史的 `users` 同库内只能存在一个，违规由 `check-db-naming.js` 拦截。
8. **OTel 启用了但不出现 trace** — `digua_common-telemetry` + `digua_common-telemetry-axum` 两个 crate 都得开 `otlp` feature，且 `OTEL_EXPORTER_OTLP_ENDPOINT` 必须设。两者缺一即为静默 stdout-only（参见 `ai-context.md` §7 坑点 11）。
9. **JWKS / 服务 JWT 不知道去哪申请** — 看 `libs/jwt-ctx` 与 `token-service` 文档，新服务调他人才走 service-jwt；新服务**自己签发 JWT 给浏览器**不在新服务职责内，那是 token-service 的活。
10. **删了/改了已发布迁移** — 全部服务处于开发期、无生产数据可以重建，**不要 amend 老迁移**，新增 `V{N+1}__*.sql`（参见 `ai-context.md` §7 坑点 5）。
11. **把 outbox 事件 publish 用 `Envelope<serde_json::Value>`** — 会被路由到 `*`，下游无法按 service 过滤。Value 仅供**消费侧**反序列化用。
12. **跨服务审计又走直连 HTTP** — Phase 4 起一律走 RabbitMQ + transactional outbox（参见 `ai-context.md` §7 坑点 19）。
13. **gateway 路由重复** — 两个 `*.router.toml` 用相同 `match` → 启动 / 重载报错（参见 [`api-gateway-routing.md`](api-gateway-routing.md) §3「冲突检测」）。
14. **`created_by` / `user_id` 写成 `BIGINT`** — 本平台 user id 是随机字符串，写入即失败或类型不匹配。新表用户引用字段统一 `VARCHAR(64)`（见 §4.3.1 与 `contracts/db-conventions.md`）。

---

## 13. 关联指南（不必重复读，按需展开）

- 路由配置：[`api-gateway-routing.md`](api-gateway-routing.md)
- 服务注册与 Consul：[`service-registration.md`](service-registration.md)
- OTel tracing：[`otel-tracing.md`](otel-tracing.md)
- 整体可观测性：[`observability.md`](observability.md)
- 灰度发布：[`canary-rollout.md`](canary-rollout.md)
- 测试：[`testing.md`](testing.md)
- AI skill（脚手架）：`.agents/skills/coder-new-service/SKILL.md`
- 加路由：`.agents/skills/coder-add-route/SKILL.md`
- 加迁移：`.agents/skills/coder-migrate-db/SKILL.md`
- 触发器总览：`.trae/rules/project_rules.md`

> 旧的 `mesh-service-onboarding.md` 描述 Consul Connect mTLS 方案，**该方案已下线**，本平台当前无 sidecar、无 mTLS。服务间调用的身份仍走 `Authorization: Bearer <jwt>`（参见 `.trae/rules/auth-rules.md`）。
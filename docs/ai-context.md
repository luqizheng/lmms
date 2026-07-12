# AI Context — coder-micro-services

> **文档性质**：状态快照 + AI 协作入口（**不是**架构教程或历史流水账）。
> AI 协作者在本仓库工作前**必须先读**本文件；它把当前事实、漂移点、未完成项收口到一处。
>
> **最后核验日期**：2026-07-12
>
> **事实来源**（已核对）：
> - `docs/ai-context.md`（本文旧版本）
> - `TODO.md`
> - 根 `README.md`、`.gitlab-ci.yml`、`infrastructure/README.md`
> - `services/*/Cargo.toml`（10 个 Rust service crate，其中包含 2 个独立 publisher crate）
> - `infrastructure/docker-compose-services.yml` + `infrastructure/` 下 compose 清单
> - `apps/pm/AGENTS.md`（含 JWT issuer/audience/scope 配置）
> - `apps/pm/pm-app-bootstrap/src/main/resources/application.yml`（JWT 配置默认值）
> - `services/project-service/README.md`、`services/project-service/AGENTS.md`
> - `services/pmd-outbox-publisher/AGENTS.md`
> - `services/config-service/src/routes/config.rs`（reveal policy 调用点）
> - `services/notification-service/src/bin/outbox_publisher.rs`（ntf-outbox-publisher binary）
> - `docs/guides/migrations-state.md`、`docs/runbooks/secret-rotation.md`
> - `docs/superpowers/plans/2026-06-25-overall-roadmap.md`
> - `docs/superpowers/specs/2026-07-11-pm-platform-design.md`、`2026-07-11-pmd-java-mvp-design.md`、`2026-07-12-pm-platform-next-steps.md`
>
> **状态口径**（全文使用，不混用其他表述）：
> - `[完成]`：代码合并 + CI 跑通 + 文档同步
> - `[部分完成]`：主体落地，但留有 spec TODO 或子任务未跑测试
> - `[待推进]`：列入 TODO 仍未动；或 spec 设计阶段尚未落地
> - `[风险]`：代码与文档、CI 与本地、设计与现实存在偏差，需要先收敛才能推进
>
> **冲突优先规则**：TODO / 实现（`Cargo.toml` / `Dockerfile` / `application.yml` / AGENTS）/ `.gitlab-ci.yml` / compose 实际配置 > 历史 roadmap / 旧 ai-context。本文档不再以旧 ai-context 为准。

---

## TL;DR（5 行）

1. **平台形态**：Vue 3 前端 + **10 个 Rust service crate**（8 个 HTTP 服务 + 2 个独立 outbox publisher）+ 2 个 Java Spring Boot 服务（`project-service` PMD、`apps/pm` 禅道替代 MVP）。`notification-service` crate 内另含 `ntf-outbox-publisher` binary。
2. **当前阶段**：12 周路线图（Phase 1-3）+ Phase 4（事件总线）+ Phase 5-A-2（registrator 替 Consul Connect）+ Phase 5-B（JWT 验签 + service-jwt）+ Phase 6-A（OTel + outbox 生产化）+ Phase 6-B（config-service dogfooding）+ Phase 8（多租户）全部 [完成]；Phase 9（Java 集成）[完成]；**Phase 10 与 Roadmap 2.0 仍有 [待推进] 项**。
3. **CI 与本地差距**：[风险] `rust_test` job 仅跑 `cargo test --workspace --lib`；pipeline 另有无 rules 的 `e2e_api_test` / `e2e_ui_test` 与按 changes 触发的 `java_sdk_test` / `dotnet_sdk_test`，但不测试两个 Java 业务服务（`services/project-service`、`apps/pm`）。CI 仍构建已废弃的 `credential-login-service` / `user-service`；`deploy` job 用不存在的根目录 `docker-compose.yml`（dev 用 `infrastructure/docker-compose.yml`）。
4. **架构冲突**：[风险] `services/project-service`（PMD，端口 8080/6008）与 `apps/pm`（端口 8088）功能高度重叠但各自演进；`apps/pm` 实现进度已领先于 spec，spec/文档存在漂移。
5. **下一优先**：见 §3 矩阵的 [待推进] 列 —— 三级健康检查、project-service 收尾、migration-state 集成测试核对、router 评估、i18n 三个 key、运维/onboarding 待补、`apps/pm` SDK 真链路打通。

---

## 1. 服务地图

### 1.1 Rust service crate（10 个）+ 同 crate 内 1 binary

`services/` 下 Rust crate 实际清单（已核对 Cargo.toml，2026-07-12）：

| Crate | 角色 | OTel feature | OTel env（compose） | 备注 |
|---|---|---|---|---|
| `api-gateway` | JWT 验签、Redis 黑名单、路由、限流 | 显式 `otlp` | 已设 `OTEL_EXPORTER_OTLP_ENDPOINT` | 唯一对外 HTTP 入口；容器 + 宿主机端口均 3000 |
| `token-service` | access/refresh/service-jwt 签发 | 显式 `otlp` | 已设 | `/tokens/.well-known/jwks.json` |
| `iam-service` | 用户 / 角色 / 权限 / 部门 / 租户；事件生产者 | **`common-telemetry` 未显式启用 `otlp`**；直接依赖 `opentelemetry` + `tracing-opentelemetry` 但 exporter 注册未独立核验 | 已设 | `iam_outbox_events` 表；`/iam/internal/*` 端点验 `aud="iam-service"`；OTel 实际导出行为待 §8 第 5 条说明 |
| `iam-outbox-publisher`（独立 publisher crate） | 轮询 `iam_outbox_events` → RabbitMQ | 显式 `otlp`（`default-features = false` + `["otlp"]`） | 未设 | 独立 crate（Phase 6-A T2-fu3 拆出），仅依赖 sqlx + common-event + common-telemetry + serde_yaml；compose 复用 `iam-service` Dockerfile 切 CMD；无 HTTP / 无 openapi.yaml / 无 migrations / 无 tests / 无自有 Dockerfile |
| `notification-service` | 公告 CRUD + WebSocket 推送 + 本地 user membership 投影 | 显式 `otlp` | 已设 | 同 crate 内另含 `[[bin]] ntf-outbox-publisher`（轮询 `ntf_outbox_events` → RabbitMQ；OTel 未设 env）；订阅 `iam.user.*` / `iam.permission.changed` / `notification.announcement.published` |
| `audit-service` | 操作日志 + 登录日志；事件消费者 | 显式 `otlp` | 未设 | RabbitMQ consumer；envelope.trace_id → span parent |
| `config-service` | 命名空间、配置项、watch；6-B dogfooding 客户端 | 显式 `otlp` | 已设 | Phase 8.3-fu2 后引入 Principal 鉴权 + namespace ownership；`is_secret` 字段见 §4.1 |
| `file-service` | 文件元数据、预签名上传、下载 | 显式 `otlp` | 已设 | VARCHAR → BIGINT 迁移（V002）；存储层见 §4.1 |
| `prue_backend` | pure-admin 菜单配置 + 动态路由 | 显式 `otlp` | 未设 | 保留 SQLite + MySQL 双栈（spec `2026-07-08-prue-backend-sqlite-decision-design.md`），`scripts/check-prue-migrations-parity.sh` 守 parity |
| `pmd-outbox-publisher`（独立 publisher crate） | Phase 9 专属，跨语言读 `pmd_outbox_events` → RabbitMQ（routing-key 前缀 `pmd.`） | 显式 `otlp`（`default-features = false` + `["otlp"]`） | 未设 | 独立 crate；compose 复用 `project-service` Dockerfile 切 CMD；无 HTTP / 无 openapi.yaml / 无 migrations / 无 tests / 无自有 Dockerfile |

> **OTel 口径（按代码 + compose 证据校准）**：
> - **代码接入**：10 个 Rust service crate 都在 `Cargo.toml` 依赖 `common-telemetry`。其中 **9 个显式开 `features = ["otlp"]`**（api-gateway / token / notification / audit / config / file / prue_backend + 两个独立 publisher `iam-outbox-publisher` / `pmd-outbox-publisher`）。**`iam-service` 走另一路径**：`common-telemetry` 与 `common-telemetry-axum` 依赖 **未显式开 `otlp` feature**，但 `Cargo.toml` 自身直接依赖 `opentelemetry = "0.24"` + `tracing-opentelemetry = "0.25"`；**是否实际注册 OTLP exporter 尚未独立核验**，不得宣称 iam-service 走自己的 exporter 路径。
> - **compose 实际导出配置**：`infrastructure/docker-compose-services.yml` 中 `OTEL_EXPORTER_OTLP_ENDPOINT=http://otel-collector:4317` 仅对 6 个 Rust HTTP 服务（api-gateway / token / iam / notification / config / file）+ `project-service` + `apps/pm` 注入。**3 个无 HTTP 出口进程未设**：`audit-service` / `prue_backend` / 两个独立 publisher crate（`iam-outbox-publisher`、`pmd-outbox-publisher`）+ `notification-service` crate 内 `ntf-outbox-publisher` binary。
> - **可观察行为差异**：compose 内未设 endpoint 的进程运行时是 stdout-only，**不会出现在 Jaeger UI**；开发/本地直跑未设 endpoint 同理。
>
> 服务通用形态：8 个 HTTP Rust crate（api-gateway / token-service / iam-service / notification-service / audit-service / config-service / file-service / prue_backend）各自有 `Cargo.toml`（含 `[[bin]] dump-openapi`）、`src/{lib,main,openapi,error,config}.rs` + `routes/` + `services/`（或 `service/`）、迁移在 `migrations/{mysql,sqlite}/`（个别服务仍用旧式扁平 `migrations/`）、`AGENTS.md`、`tests/*.rs`、`Dockerfile`。两个独立 publisher crate（`iam-outbox-publisher`、`pmd-outbox-publisher`）**不含这些**：无 dump-openapi binary、无 HTTP 路由、无 openapi.yaml、无 migrations、无 `AGENTS.md`（个别有豁免 README）、无 `tests/`（运行时行为由下游 consumer + e2e 覆盖）；`pmd-outbox-publisher` 仅运行时存活 + 由 Compose 重启，`iam-outbox-publisher` 同上。`notification-service` crate 内另含 `[[bin]] ntf-outbox-publisher`（轮询 `ntf_outbox_events` → RabbitMQ；同 crate 不重复 `dump-openapi`、不入本表 publisher 行；OTel env 未设），共享 crate 的 HTTP 形态。

### 1.2 Java 业务服务（2 个）

| 服务 | 端口（容器 / 宿主机） | 技术栈 | 状态 |
|---|---|---|---|
| `services/project-service` | 8080 / 6008 | Spring Boot 3 + Java 25 + JPA + Flyway + springdoc-openapi | [部分完成] Phase 9 验证 MVP，事件生产者（Java `OutboxWriter`）；Rust `pmd-outbox-publisher` 跨语言发布；`pmd_outbox_events` 同事务入队；占位 LLM `PlaceholderLlmProvider`；3 项收尾见 §4.1 |
| `apps/pm` | 8088 / — | Spring Boot 3 + MyBatis-Plus + Flyway + springdoc-openapi + flexmark-java | [部分完成] **README 自称 v0.1.0 MVP**（v1-v5 共 5 个 Flyway 迁移：pm_/kb_/task_/sprint_/comment_；含 `openapi.yaml`）；RS256 JWKS（默认 `PM_JWT_MODE=jwks`）**已验证配置**（`PM_JWT_ISSUER=superpowers-token-service` / `PM_JWT_AUDIENCE=coder-micro-services` / `PM_JWT_REQUIRED_SCOPE=access`，HS256 + dev `noop` 兜底）；LLM 默认 `noop`，可通过 `PM_LLM_PROVIDER=deepseek` + `DEEPSEEK_API_KEY` 启用 OpenAI-compatible DeepSeek；compose 集成 `OTEL_EXPORTER_OTLP_ENDPOINT`。**注：相关 spec `2026-07-11-pm-platform-design.md` 状态仍标"设计阶段"，实际实现已领先于 spec；spec 仍遗留发布/E2E/其他 service SDK 集成等 follow-up（参见 2026-07-12 follow-up spec）** |

> **架构冲突（[风险]）**：`project-service` 与 `apps/pm` 都是"研发管理工具"，但产品定位、数据模型、演进路径互不收敛。详见 §7「文档与架构冲突」。AI 在这两处动代码前必须先确认走哪条线。

### 1.3 基础设施（docker compose）

| 组件 | 容器端口 | 宿主机端口 | 角色 |
|---|---|---|---|
| consul | 8500 / 8502 | 8500 | 服务发现 + Consul SD（Prometheus 用） |
| redis | 6379 | 6379 | JWT 黑名单 / 缓存 / 限流（Phase 10 改走 Redis） |
| mysql | 3306 | 3306 | 共享库 `superpowers`，表名按服务前缀（Phase 4.0 落地） |
| rabbitmq | 5672 / 15672 | 5672 / 15672 | 事件总线 broker；拓扑由 `infrastructure/config/rabbitmq/definitions.json` 预声明 |
| registrator | — | — | `gliderlabs/registrator:master`，项目级服务注册（**Phase 5-A-2 替代了原 6 个 sidecar + consul connect proxy -register**） |
| prometheus | 9090 | 9090 | 单一 `prometheus.yml`（consul SD + 自抓 + otel-collector） |
| grafana | 3000 | 3001 | 仅预置 `gateway.json` 一个 dashboard；admin/admin |
| jaeger | 16686 | 16686 | Trace UI |
| otel-collector | 4317 / 4318 / 8889 | 同 | OTLP gRPC / HTTP + Prometheus exporter |

> **compose 文件分层**（`infrastructure/docker-compose*.yml` 5 个，详见 `infrastructure/README.md`）：
> - `docker-compose.yml`：主入口，`include` 下面三个
> - `docker-compose-infra.yml`：基础设施基线（无端口发布、无 dev 覆盖）
> - `docker-compose-infra.dev.yml`：dev overlay（加端口发布）
> - `docker-compose-services.yml`：8 个 Rust 服务 + frontend + 2 个 outbox publisher
> - `docker-compose-observability.yml`：可观测性 overlay
>
> **端口两套并存**：宿主机直接 `cargo run` 默认 6001-6007（见根 README「服务端口与依赖」）；compose 容器内 808x-9081（见 `infrastructure/README.md`「端口映射」）。两套互不干扰。

---

## 2. 关键事实速查（AI 5 分钟可读完）

- **架构**：浏览器 → `api-gateway:3000` → Rust 业务服务；服务间调用 `Authorization: Bearer <service-jwt>` 透传（无 mTLS / 无 SPIFFE）；事件总线走 RabbitMQ + transactional outbox。
- **事件总线**：每个生产者服务一张 `*_outbox_events` 表（同事务 enqueue），独立 `*-outbox-publisher` 进程/crate 轮询发布；消费者按 `event_id` 幂等；at-least-once 语义。**首批事件**：`iam.user.created` / `iam.user.deleted` / `iam.login.succeeded` / `iam.login.failed` / `iam.permission.changed` / `iam.tenant.created` / `notification.announcement.published` / `iam.user.role.{assigned,unassigned}` / `iam.user.dept.changed` / `pmd.*`（Phase 9）。
- **JWT**：RS256；网关验签；下游用 `libs/common-jwt::JWKSCache`；`Principal` extractor 取身份（`libs/jwt-ctx`）；**禁止读 `X-User-*` headers**。
- **多租户**：共享 MySQL + `tenant_id` 列；JWT `tid` claim + `tenant_id` 必填（缺则 401）；`cross_tenant: bool` 给 platform-admin；`scripts/check-tenant-scope.js` lint 守门。
- **OpenAPI**：8 个 Rust HTTP 服务（含 api-gateway）+ 2 个 Java 服务，共 **10 个当前提交/暴露的 OpenAPI**。8 个 Rust HTTP 服务的 `openapi.yaml` 由 utoipa 自动生成，2 个 Java 服务（`project-service` / `apps/pm`）的 `openapi.yaml` 由 springdoc-openapi 自动生成。两个独立 publisher crate（`iam-outbox-publisher`、`pmd-outbox-publisher`）无 HTTP、无 `openapi.yaml`。**CI `spec_generate` 的 glob `services/*/openapi.yaml` 不命中 `apps/pm/openapi.yaml`**（路径不在 `services/` 下），后者由 springdoc 生成但不被 Rust drift gate 把守。所有 yaml 禁止手改，源头改注解后重生成。
- **迁移**：forward-only；prue_backend 同时维护 MySQL + SQLite 两个版本；其余仅 MySQL；`scripts/check-db-naming.js` 强制表名前缀（`iam_/audit_/token_/ntf_/cfg_/file_/prue_/pm_/kb_/pmd_/task_/sprint_/comment_`）。
- **契约层**：`contracts/error-codes.yaml` / `permissions.yaml` / `events.yaml` / `http-conventions.md` / `db-conventions.md` 单一真相源。`scripts/contracts-gen.js` 生成 `libs/common-error/src/codes.rs`、`libs/authz/src/perms.rs`；**禁止手改生成文件**。

---

## 3. 阶段矩阵（Phase 1-10 + Roadmap 2.0）

> 数据来源：TODO.md 全部勾选状态 + 旧 ai-context.md + `2026-06-25-overall-roadmap.md`。

| Phase | 主题 | 状态 | 关键证据 |
|---|---|---|---|
| 1 | 服务规范 sweep + prue 并发 + active routes 净化 | [完成] | ADR-002 + 8-item baseline + 3 CI gates；spec `2026-06-25-prue-backend-concurrency-design.md` |
| 2 | Swagger 全覆盖 / OpenAPI auto-dump / skills / L4 triggers / 前端 P1 | [完成] | 8/8 Swagger 覆盖；`scripts/dump-openapi-specs.sh`；`.agents/skills/{coder-add-route,coder-migrate-db,coder-new-service}`；`.trae/rules/project_rules.md` |
| 3 | 可观测性 + 限流 + E2E 稳定性 + 灰度 + 知识沉淀 | [完成] | Prometheus + OTel scaffold；api-gateway 集中限流；smoke 子集；Nginx + Consul canary |
| 4 | 异步事件总线（RabbitMQ + outbox） | [完成] | 17 任务全部完成；4.1 welcome / 4.2 登录风控 / 4.3 announcement WS / 4.3-fu1~fu4 / 4.4 permission-changed + perm-epoch / 4.4-fu1~fu4 |
| 5-A-2 | 撤 Consul Connect，改 registrator | [完成] | spec `2026-07-07-deprecate-consul-connect-use-registrator-design.md`；10 commit |
| 5-B | JWT 验签 + service-jwt + 通知事件化 | [完成] | spec `2026-07-07-phase5b-jwt-signature-and-service-jwt-design.md`；5B-fu1~fu6 落地 |
| 5-A（原始 mTLS 选型） | 撤并至 registrator | [完成]（撤销） | Phase 5-A-2 撤回后，spec `2026-07-03-phase5a-consul-connect-mtls-design.md` 仅作历史 |
| 5-C | RabbitMQ/MySQL/Redis TLS | [待推进]（→ R2.1） | 1.0 不增加安全功能；R2.0 统一规划 |
| 6-A | Trace 链闭环 + 事件总线生产化 | [完成] | T1 trace_id 自动注入 / T2 OTel 全覆盖 + tracing_mw 提取 / T3 transient/permanent + bounded redelivery / T4 24h archive；follow-up T2-fu1~4 + T3-fu1~4 + T4-fu1 已落，**P2 指标 `iam_outbox_archived_total` / `iam_outbox_purged_total` 因 outbox-publisher 无 `/metrics` 推迟** |
| 6-B | config-service dogfooding | [完成]（既定范围） / [部分完成]（平台扩面） | T1 `ConfigServiceProvider` / T2 `Loader` + watch / T3 **token-service 首接入**落地；其余业务服务 / 独立 publisher / 前端尚未全面 dogfood，仍直接读 `config.yaml` / env |
| 7 | K3s / 灰度发布升级 | [待推进] | chart/manifest 未写；三级探针待补；Argo Rollouts/Flagger 未评估 |
| 8 | 多租户 | [完成] | 8.1 骨架 / 8.2 业务表 tenant_id 迁移 / 8.3 全面贯通 + cross_tenant flag / 8.3-fu1 iam 点查询 / 8.3-fu2 ntf/cfg/file 点查询；`scripts/check-tenant-scope.js` 启发式升级 [部分完成]（沿用 delegated 豁免） |
| 9 | Java 集成示例 + audit SDK + project-service | [完成]（示例）/[部分完成]（project-service 收尾） | `examples/java8-webapi/` + `sdk/java/audit-service-sdk` 30 测试全过；`project-service` Spring Boot 3 + Java 25 PMD MVP；**生产化收尾见 §4.2** |
| 10 | 小问题清单（收尾） | [部分完成] | 详见 §4.1 |
| R2.0 | TLS termination / RabbitMQ/MySQL/Redis TLS | [待推进] | 阻塞：1.0 落地 + 至少 1 个外部租户实际接入并提出合规 |

---

## 4. 当前未完成项

### 4.1 Phase 10 [部分完成]（TODO.md 现存 15 项中按主题归类）

**健康检查**：
- 三级探针（startup / liveness / readiness）尚未实现：当前 api-gateway 在 compose 层有 healthcheck，其余 7 个 Rust 服务 Dockerfile 无 `HEALTHCHECK` 指令；`docker ps` 只显示 "Up"。不影响启动（没有 `condition: service_healthy` 依赖），但与 pm / project-service 行为不一致。
- migration-state 端点（`/<svc>/internal/migrations/state`）：**已核对 `docs/guides/migrations-state.md` 为 v1，Phase 10 G3 落地**，列 7 个 Rust 服务（iam V001-V005 / notification V001-V003 / token / audit / config V001-V002 / file V001-V002 / prue_backend V001 × 2 dialects）。`TODO.md` 仍把"集成测试 + 文档同步"列在 [ ] — 矛盾点：guide 已存在但 TODO 未勾选。**待核对**：① 各服务端点集成测试是否补；② `TODO.md` 该条目是否同步勾选；③ 各服务 `AGENTS.md` 是否引用 guide。

**project-service 收尾**（2026-07-12 docker compose 修复 session 遗留）：
- `services/project-service/pom.xml` 缺 `spring-boot-starter-test` test scope —— 当前 Dockerfile 用 `-Dmaven.test.skip=true` 绕过测试编译。
- 外部 `config/application.yml` 用 `${DATABASE_URL}` / `${DB_USER}` / `${DB_PASSWORD}` 占位符，但 Spring 实际读 `SPRING_DATASOURCE_*` 环境变量，两个 `${}` 是死代码。
- `services/project-service/maven-settings.xml` 应审视（aliyun mirror 在非 CN / 内部 CI 不一定适用；候选：build-arg 或 mount `~/.m2/settings.xml`）。

**config-service 缺口**（实际代码复核需跟进）：
- `services/config-service/src/routes/config.rs` 已实现 `apply_reveal_policy` + 常量 `REVEAL_PERM = "CFG_CONFIG_REVEAL"`，在 6 个 handler（list / create / get / patch / delete 等）调用点对 `is_secret=true` 项做掩码，`principal.has_permission(REVEAL_PERM)` 决定是否透明。代码侧 reveal policy 已落地。**风险点（需独立核对）**：① `CFG_CONFIG_REVEAL` 权限码是否已在 `contracts/permissions.yaml` 注册并与 iam-service role 绑定；② `apply_reveal_policy` 在所有 6 个调用点的集成测试是否补；③ `config-service/AGENTS.md` 是否同步说明该行为。

**file-service 存储层缺口**：
- 实际仅实现 `LocalStorage`（实现 `storage/object_store::ObjectStore` trait）；`S3Storage` / `RustfsStorage` / 其他 object store adapter **尚未实现**。生产部署若需 S3 兼容对象存储，需补对应 implementation 并在 `services/file-service/src/storage/{s3,rustfs}.rs` 实现 trait + 注册到 `storage/mod.rs`。

**评估项**：
- 评估自研 router 替换为 Envoy / Pingora 的成本/收益（待启动）。

**前端遗留**：
- i18n 三个 key 缺失（`commit 20488c2` 把 359 keys 从 `buttons:` 迁到 `menus:` namespace 后，扫到 `menus.accountProfile` / `menus.accountSecurity` / `menus.accountSettings` 三个 key 在 yaml 中不存在；fallback 显示原始字符串）。修复方案 A（加 alias key）或 B（改用 `pureProfile` / `pureSecurityLog` / `pureAccountSettings`）二选一，**本 session 不动**。

**指标**：
- `iam_outbox_archived_total` + `iam_outbox_purged_total` IntCounter [推迟] —— `iam-outbox-publisher` 现为独立 CLI binary，无 `/metrics` HTTP 端点；Prometheus counter 注册了也抓不到。

**`apps/pm` 跟真 IAM 链路未打通**：
- `apps/pm` SDK 默认 `PM_SDK_BEARER=${PM_SDK_BEARER:-dev-bypass-token}`，为 dev 占位 token，与 iam-service 颁发的真 JWT 不互通；以当前配置跑 `docker compose up`，`apps/pm → audit-service` 等 SDK 调用会被 audit-service 鉴签 401。后续 PR 计划：① SDK 模块发布到 `mvn.kudi.repos`；② 真链路打通（Bearer 改 IAM token）；③ 其他 5 个 PM service（KbSpace / KbDocument / RequirementService 等）按 `ProjectService` 模板接入 audit。

### 4.2 Onboarding / 运维 [待推进]

来自 `docs/onboarding/` 系列 TODO：
- 内网仓库凭证申请流程（Docker Registry / Maven / NuGet）—— 待运维补充
- 生产数据迁移 SOP（备份 / 回滚 / 灰度验证）—— 待运维补充
- 集中式日志收集方案（ELK / Loki / 阿里云 SLS）—— 待运维补充
- 求救渠道 / on-call 值班表 —— 待团队负责人补充
- commit message 规范（是否走 Conventional Commits / PR 标题 / issue 关联）—— 待规范人补充
- config-service 使用方式 —— 待熟悉 config-service 的开发者补充
- 前端登录态刷新机制细节 —— 待前端负责人确认

### 4.3 Roadmap 2.0 [待推进]

- R2.1 基础设施层 TLS（RabbitMQ AMQP / MySQL / Redis）
- R2.2 TLS termination（alb / nginx 终结）
- 阻塞条件统一：1.0 落地 + 至少 1 个外部租户实际接入并提出合规需求

---

## 5. CI 与本地差距（[风险]）

来源：`.gitlab-ci.yml` 已核对全文（2026-07-12）。`stages` 仅列 `test / build / deploy / publish` 四段。

### 5.1 CI 当前行为

| 阶段 | Job | 触发 | 实际跑什么 | 差距 / 风险 |
|---|---|---|---|---|
| `test` | `rust_test` | 所有 pipeline | `cargo test --workspace --lib --verbose` | [风险] 仅 lib 单元测试；不跑 `services/<svc>/tests/` 集成测试、不跑 E2E、不跑 Java SDK、不跑 `apps/pm` mvn verify |
| `test` | `service_standard_check` | 所有 pipeline | `node scripts/check-service-standard.js` | 覆盖 `services/**`（不覆盖 `apps/pm`） |
| `test` | `spec_check` | 所有 pipeline | `node scripts/check-spec.js` | 校验 `services/*/openapi.yaml`（不覆盖 `apps/pm/openapi.yaml`） |
| `test` | `contracts_check` | 所有 pipeline | `node scripts/contracts-gen.js --check` | 校验 `contracts/**` 生成代码未漂移 |
| `test` | `spec_generate` | 依赖 `spec_check` | `scripts/dump-openapi-specs.sh` + `git diff --exit-code -- 'services/*/openapi.yaml'` | 守 Rust openapi drift；**glob 限定 `services/*/openapi.yaml`，不守 `apps/pm/openapi.yaml`**（由 springdoc 生成，不会进入 Rust drift gate） |
| `test` | `e2e_api_test` | 所有 pipeline（无 `rules:`） | `cd tests/e2e && pnpm run test:api` | 跨网关测真服务 |
| `test` | `e2e_ui_test` | 所有 pipeline（无 `rules:`） | `cd tests/e2e && pnpm run test:ui`（Playwright） | 需前端容器 |
| `test` | `java_sdk_test` | `changes: sdk/java/**` | `cd sdk/java && mvn -B verify` | 仅 SDK 模块改动时触发；**`services/project-service` / `apps/pm` mvn 测试不在 CI 范围** |
| `test` | `dotnet_sdk_test` | `changes: sdk/dotnet/**` | `cd sdk/dotnet && dotnet test` | 同上 |
| `build` | `build_api_gateway` | 所有 pipeline | `Dockerfile.api-gateway` → `docker push` | — |
| `build` | `build_token_service` | 所有 pipeline | `Dockerfile.token-service` → push | — |
| `build` | `build_credential_login_service` | 所有 pipeline | `Dockerfile.credential-login-service` → push | **[风险] credential-login-service 已废弃并合并进 iam-service**，CI 仍构建并推送镜像 |
| `build` | `build_user_service` | 所有 pipeline | `Dockerfile.user-service` → push | **[风险] user-service 已废弃并合并进 iam-service**，CI 仍构建 |
| `deploy` | `deploy` | `only: main` | `cp -r . ~/coder-micro-services` → `docker-compose -f docker-compose.yml down` → `pull` → `up -d` | **[风险] ① 非零停机（先 down 再 up）；② 路径漂移 —— 仓库根 `docker-compose.yml` 当前不存在（已核对 `ls`），实际 compose 文件均在 `infrastructure/`；该 job 现在无法成功运行** |
| `publish` | `sdk_publish` | `if: $CI_COMMIT_TAG =~ /^v\d+\.\d+\.\d+$/` | `mvn -B deploy -DskipTests` | tag 触发 |

### 5.2 凭据明文（[风险] —— 必须立即轮换，不要在文档复述值）

`.gitlab-ci.yml` 中以明文形式出现的 variables（2026-07-12 已核对）：
- `DEPLOY_PASSWORD`（部署目标服务器密码）
- `E2E_DB_PASSWORD`（E2E 测试 MySQL 密码）

> 凭据具体值**不在本文档复述**。处理建议：① 立即把上述字段迁出 yml，改由 GitLab CI/CD `variables:` / `Settings > CI/CD > Variables` 注入并启用 Protected + Masked；② `infrastructure/.env` 同源默认凭据一并轮换；③ `deploy` job 改为按服务滚动或蓝绿，避免 `down`/`up` 中断；④ 修 `deploy` job 的 compose 路径指向 `infrastructure/docker-compose.yml` 或拆出专用 `docker-compose.deploy.yml`。

---

## 6. 可观测性现状（与文档不一致）

| 组件 | 当前实际 | 文档期望 | 差距 |
|---|---|---|---|
| Prometheus | 单一 `infrastructure/config/prometheus/prometheus.yml`（consul SD + 自抓 + otel-collector） | — | [正常] |
| Grafana | 仅预置 1 个 dashboard：`infrastructure/config/grafana/dashboards/gateway.json` | 旧 README「dashboards 自动加载」给人多面板印象 | [风险] 与文档措辞不一致；只覆盖 api-gateway，其他服务的 metrics 面板需自建 |
| Grafana provisioning | `datasources/prometheus.yml` + `dashboards/defaults.yml` | — | [正常] |
| runbooks | `docs/runbooks/secret-rotation.md`（**仅此一份**） | — | [待推进] 故障排查 / 灾备 / 回滚 runbook 尚未建 |
| OTel 端到端 trace | 9 个 crate 显式开 `otlp` feature；iam-service 未显式开（直接依赖 `opentelemetry`/`tracing-opentelemetry`，但 exporter 注册未核验）；compose 只对 6 个 Rust HTTP 服务 + 2 个 Java 服务设置 `OTEL_EXPORTER_OTLP_ENDPOINT` | 旧 ai-context "4 个服务 OTel"；旧根 README「服务端口与依赖」表 OTel 列均按当时状态，已与 Cargo.toml + compose 不一致 | [纠正] 区分"代码接入"与"compose 实际导出配置"；详见 §1.1 表格 |

> **OpenAPI drift 实际范围（不一致点）**：CI `spec_generate` job 的 `git diff --exit-code -- 'services/*/openapi.yaml'` **只覆盖 Rust 服务由 utoipa 生成的 yaml**；`apps/pm/openapi.yaml`（由 springdoc-openapi 自动生成）**不在该 glob 内**，其漂移当前不被 CI 把守。apps/pm spec 也未将 OpenAPI drift 列入自家门禁。

---

## 7. 文档与架构冲突（[风险]）

### 7.1 `services/project-service` vs `apps/pm`

两个 Spring Boot 3 Java 服务并存，**功能重叠、定位不清**：

| 维度 | `services/project-service` | `apps/pm` |
|---|---|---|
| 端口 | 容器 8080 / 宿主机 6008 | 容器 8088 |
| 技术栈 | Spring Boot 3 + **Java 25** + **JPA** + Flyway + springdoc-openapi | Spring Boot 3 + MyBatis-Plus + Flyway + springdoc-openapi + flexmark-java |
| 数据模型 | `pmd_*` 前缀（项目 / 里程碑 / 任务 / sprint / LLM 细化） | `pm_*` / `kb_*` / `task_*` / `sprint_*` / `comment_*` 前缀（项目 / 需求 / 用户故事 / AC / 知识库 / 任务 / 迭代 / 评论） |
| LLM | `PlaceholderLlmProvider`（无 HTTP） | `NoopLlmProvider`（默认）；`PM_LLM_PROVIDER=deepseek` + `DEEPSEEK_API_KEY` 启用 DeepSeek（OpenAI-compatible） |
| JWT | **README/AGENTS 未明确当前 inbound JWT 实现，需读代码核对**（`AGENTS.md` 未声明 `PM_JWT_MODE` / `PM_JWT_ISSUER` 等字段；HS256 自签 vs RS256 JWKS 等选项未在两份文档内现成答案） | `PM_JWT_MODE` 默认 `jwks`（RS256 + 共享 iam-service JWKS，`PM_JWT_ISSUER=superpowers-token-service` / `PM_JWT_AUDIENCE=coder-micro-services` / `PM_JWT_REQUIRED_SCOPE=access`，HS256 + dev `noop` 兜底） |
| 事件总线 | `pmd_outbox_events` 表 + Java `OutboxWriter` + Rust `pmd-outbox-publisher` | 无 outbox（不通过 RabbitMQ 跨服务通信） |
| OpenAPI | springdoc-openapi → `services/project-service/openapi.yaml` | springdoc-openapi → `apps/pm/openapi.yaml`（**不在 Rust drift gate glob 内**） |
| 状态 | [部分完成] Phase 9 验证 MVP；TODO 列 3 项收尾；spec `2026-07-11-pmd-java-mvp-design.md` / `2026-07-11-pmd-mvp-design.md` | [部分完成] README 自称 v0.1.0 MVP；spec `2026-07-11-pm-platform-design.md` **仍标"设计阶段"**（与实际实现不同步）；follow-up spec `2026-07-12-pm-platform-next-steps.md` |
| 已验证实现 | Service（Sprint / Task）+ Flyway 迁移 | 5 个 Flyway 迁移（v1 pm_ / v2 kb_ / v3 task_ / v4 sprint_ / v5 comment_）；6 个 PM service + KB service + AuditEventReporter + 16 处真实 audit.report(...)；RS256 JWKS 解码 + dev noop；flexmark-java Markdown 渲染；AuditEventReporterTest 单元测试 |
| SDK 链路 | — | SDK wiring 已配置；**真实 audit 上报链路在 compose up 后仍未打通**（默认 `PM_SDK_BEARER=dev-bypass-token`，因 dev-bypass-token 非 iam-service 颁发真 token，audit-service 当前会回 401）；SDK 模块发布 / E2E / 其他 service 接入仍按 apps/pm README 列在后续 PR |

**根 README 表述混乱**：根 README「Services」表把两者混为一谈（"PM Platform" 一段既指 `apps/pm` 又指 `project-service`）。

**AI 工作要求**：动其中任一服务前，**必须**先确认：
- 需求落在哪条线上（PMD 路线 / 禅道替代路线）
- 是否走事件总线（`project-service` 走；`apps/pm` 不走）
- 是否需要 IAM 真 token 调下游（`apps/pm` 当前 dev-bypass-token 与 iam 颁发的 JWT 不互通 → 真链路 audit 401）
- 是否需要在 `scripts/check-db-naming.js` 加前缀（`pmd_` / `pm_` / `kb_` / `task_` / `sprint_` / `comment_`）

### 7.2 OTel 覆盖 vs compose env

旧 ai-context.md §7 第 11 条 + 根 README「服务端口与依赖」表 OTel 列均标 audit/config/file/prue_backend 为 "—"。**已与代码 + compose 不一致**：10 个 Rust crate 均在 `Cargo.toml` 依赖 `common-telemetry`（9 个显式开 `features = ["otlp"]`，iam-service 未显式开、exporter 注册未核验）；是否真的导出 span，**还需** compose `OTEL_EXPORTER_OTLP_ENDPOINT` 已设置 —— 当前仅 6 个 Rust HTTP 服务 + 2 个 Java 服务注入。详见 §1.1 表格。

### 7.3 部署 compose 路径（[明确漂移]）

`.gitlab-ci.yml` `deploy` job 用的是仓库根 `docker-compose.yml`。**根目录列表已核对不存在该文件**（仅有 `docker-example/`、`infrastructure/` 两个含 compose 文件的目录，与 `deploy` job 用的根路径不匹配）。dev 流程用的是 `infrastructure/docker-compose.yml`（含 `include` 3 个子文件）。**该 `deploy` job 当前配置下没有可运行的 compose 文件 —— 部署必然失败**。修法候选：① deploy job 改用 `docker-compose -f infrastructure/docker-compose.yml`；② 补一份仓库根 `docker-compose.yml` 镜像生产部署形态；③ CI 改走 K8s/manifest（对应 Phase 7 [待推进]）。

---

## 8. 关键约定与坑点（仍准确，保留）

> 以下是经实测仍生效的约定，新 AI 协作者请遵守。

1. **`utoipa::OpenApi::to_yaml()` 需要 `yaml` feature**：`Cargo.toml` 必须 `utoipa = { version = "5", features = ["axum_extras", "chrono", "yaml"] }`。缺了 `ApiDoc::openapi().to_yaml()` 编译不过。

2. **本地 `cargo` 编译可能因 `cc` 链接器缺失失败**：用 `node scripts/run-tests-docker.js <svc>` 或在 `zhcoder-docker-registry.com:8000/builder/rust:1.96-slim-musl` 容器内构建。

3. **`api-gateway` 黑名单测试是历史遗留失败**：它要求宿主机 Redis 在 `127.0.0.1:6379`；与任何在途改动无关。Docker 跑（用测试 Redis）可通过。

4. **手写 `openapi.yaml` 已废止**：全部 10 份由 utoipa / springdoc 自动生成。改 spec 改代码（`#[utoipa::path]` / `@Operation`），再跑 `scripts/dump-openapi-specs.sh`。**不要直接编辑 yaml**。

5. **OTel traces 靠 env + feature 双开**（口径已按 Cargo.toml + compose 实际校准）：
   - 9 个 Rust service crate 在 `Cargo.toml` 显式 `features = ["otlp"]`：`api-gateway` / `token-service` / `notification-service` / `audit-service` / `config-service` / `file-service` / `prue_backend` + 两个独立 publisher `iam-outbox-publisher` / `pmd-outbox-publisher`。
   - `iam-service` 走另一路径：`common-telemetry` 与 `common-telemetry-axum` 依赖未显式开 `otlp` feature；`Cargo.toml` 自身直接依赖 `opentelemetry = "0.24"` + `tracing-opentelemetry = "0.25"`，**但是否实际注册 OTLP exporter 尚未独立核验**。
   - compose 运行期 `OTEL_EXPORTER_OTLP_ENDPOINT=http://otel-collector:4317` 仅对 6 个 Rust HTTP 服务（`api-gateway` / `token` / `iam` / `notification` / `config` / `file`）+ `project-service` + `apps/pm` 设置。
   - **3 类进程 compose 未设 env**：audit-service / prue_backend / 两个独立 publisher crate（`iam-outbox-publisher`、`pmd-outbox-publisher`）+ `notification-service` crate 内 `ntf-outbox-publisher` binary。运行时为 stdout-only tracing。
   - 任意一端（feature 或 env）缺失 → 进程不导出 span 到 OTel collector（iam-service 是否导出于实际 init 路径未独立核验）。判定方法：启动后查日志是否出现 `OTLP exporter` / tracer provider 初始化行；或在 Jaeger UI 搜不到对应 service / binary。

6. **迁移是 forward-only**：不要编辑已发布迁移；新增 `V{N+1}__*.sql`。`scripts/check-prue-migrations-parity.sh` 守 prue_backend MySQL/SQLite parity。

7. **JWT 上下文只用 `Authorization` header**（`.trae/rules/auth-rules.md`）。任何地方**禁止**读 `X-User-*` headers。用 `libs/jwt-ctx::Principal` extractor。

8. **`check-spec.js` 报缺少 `summary` 是 warning 不是 error**；exit 0。utoipa 默认不生成 summary。

9. **`scripts/contracts-gen.js --check` 会重新生成 `libs/common-error/src/codes.rs`**：不要直接编辑该文件。改 `contracts/error-codes.yaml`，再重跑。

10. **`notification/ws` 鉴权**：网关验 JWT 并注入用户身份；浏览器在 WS upgrade 时自动带 `Authorization`。**不要把 token 放 URL 里**。

11. **网关限流返回 429 + `Retry-After`**：查 `infrastructure/config/api-gateway/rate-limits.toml` 看按路由覆盖；默认 100 rps / 200 burst；`/v1/iam/auth/login` 5 rps / 10 burst。Phase 10 起 gateway 限流统一走 Redis（commit `ddf44a0`），淘汰 in-memory 实现。

12. **服务侧限流经 `common-ratelimit` opt-in**：Redis 滑动窗口、跨副本共享、Redis 出错时 fail-open。模式见 `services/notification-service/src/rest.rs`。

13. **灰度通过 Consul `canary` tag**：consul tag + Nginx `split_clients` 接 ~10% 流量；用 `?canary=1` 或 `X-Canary: true` 强制路由。详见 `docs/guides/canary-rollout.md`。

14. **E2E Smoke 子集用 `E2E_SMOKE=1`**：只跑带 `smoke: true` 的用例，~2 分钟；CI 跑全量。详见 `docs/guides/testing.md`。

15. **共享 MySQL `superpowers` 库表必须带服务前缀**（Phase 4.0）：`iam_/audit_/token_/ntf_/cfg_/file_/prue_/pm_/kb_/pmd_/task_/sprint_/comment_`。`scripts/check-db-naming.js` 强制。

16. **事件总线 = RabbitMQ + transactional outbox**（Phase 4）。同事务写业务行 + `common_event::MySqlOutboxRepo::enqueue(&mut tx, &Envelope::new(...))`；独立 `*-outbox-publisher` 进程轮询发布；消费者按 `event_id` 幂等。**`Envelope<serde_json::Value>` 仅用于消费**，publish 必须用强类型 Event。

---

## 9. 常用验证命令（push 前必跑）

```bash
# CI 门禁本地复刻
node scripts/check-service-standard.js      # services/** 服务规范
node scripts/check-spec.js                  # OpenAPI / 错误码 / 权限码
node scripts/contracts-gen.js --check       # contracts/** 生成代码未漂移
node scripts/check-db-naming.js             # 表前缀
scripts/dump-openapi-specs.sh               # 重新生成 openapi.yaml
git diff --exit-code -- 'services/*/openapi.yaml'  # drift 守门

# Rust 单元测试（docker 内）
node scripts/run-tests-docker.js audit-service config-service file-service prue_backend iam-service notification-service token-service api-gateway

# E2E
pnpm -C tests/e2e install
pnpm -C tests/e2e test:smoke        # ~2 分钟 smoke
pnpm -C tests/e2e test:api          # 全量 API（`e2e_api_test` job 无 rules，会进入 pipeline）
pnpm -C tests/e2e test:ui           # 全量 UI（需前端容器）

# 可观测性
docker compose -f infrastructure/docker-compose.yml up -d
# Prometheus :9090   Grafana :3001（仅 gateway dashboard）   Jaeger :16686
# OTel collector OTLP gRPC :4317

# Java（按需）
cd services/project-service && ./mvnw test         # 受限：缺 test scope
cd apps/pm && mvn -Pjava17 clean verify            # MVP 阶段仅 compile + boot
cd sdk/java && mvn -B verify                       # 仅 rules.changes: sdk/java/** 触发
```

---

## 10. 导航地图（按意图）

| 意图 | 入口 |
|---|---|
| 加路由 / 加迁移 / 新建服务 | `.agents/skills/coder-{add-route,migrate-db,new-service}/SKILL.md` |
| 改契约（错误码 / 权限码 / 事件） | `contracts/*.yaml` + `node scripts/contracts-gen.js` |
| 评审 plan | `.trae/skills/plan-eng-review/SKILL.md` |
| 审计服务规范 | `.trae/skills/coder-service-audit/SKILL.md` |
| 调试请求链路（OTel / Jaeger） | `docs/guides/otel-tracing.md` |
| 灰度发布 | `docs/guides/canary-rollout.md` |
| 搭可观测性 | `docker compose -f infrastructure/docker-compose.yml up -d` + `docs/guides/observability.md` |
| 跑 / 写测试 | `docs/guides/testing.md` |
| 凭据轮换 SOP | `docs/runbooks/secret-rotation.md` |
| 某服务的约定 | `services/<svc>/AGENTS.md` 或 `apps/pm/AGENTS.md` |
| ADR | `docs/decisions/ADR-*.md` |
| 阶段总览 | `docs/superpowers/plans/2026-06-25-overall-roadmap.md` + `TODO.md` |
| 新人入门 | `docs/onboarding/README.md` |

---

## 11. 维护规则

> 本文档的存续意义是**让下次 AI / 开发者 5 分钟内拿到当前事实**。请按以下规则同步更新：

1. **何时更新**：
   - 服务增减 / 端口变更 / 容器 compose 改动 → 同步更新 §1 服务地图
   - 阶段状态变化（[完成] / [部分完成] / [待推进]） → 同步更新 §3 矩阵
   - 新增 TODO 项或 TODO 项关闭 → 同步更新 §4（同时改 `TODO.md`）
   - CI job / stages / 凭据变量变化 → 同步更新 §5
   - 可观测性组件增减 / dashboard 增减 → 同步更新 §6
   - 服务间定位冲突或集成方式变化 → 同步更新 §7
   - 任何被新代码推翻的"事实" → 同步更新 §8 坑点

2. **冲突优先规则**（重申）：
   - **TODO.md（实时债务清单） > 实现（`Cargo.toml` / `application.yml` / Dockerfile / 源码 / 各 `AGENTS.md`） > `.gitlab-ci.yml` + `infrastructure/docker-compose*.yml` 实际配置 > 历史 roadmap（仅回顾） > 本文档旧版本**。
   - 当任意三类源冲突时：以**最具体的实现证据**（如 `Cargo.toml` 的 feature flag、`docker-compose*.yml` 的 env 注入）为准。
   - 文档与代码冲突时：**改文档**，并在 commit message 注明。

3. **禁止**：
   - 复述 `.gitlab-ci.yml` 或任何文件中的明文凭据具体值
   - 把"设计阶段"或"规划中"的服务写成"已上线"；反向同理（已上线的服务也不必说成"设计阶段"）—— 状态要按实际实现描述，spec 状态滞后于实现时单列"spec 漂移"
   - 宣称未运行的测试已通过
   - 把历史 spec / 已废弃 ADR 当作"当前事实"
   - 增加 emoji（用 `[接入]` 等纯文字状态标签代替）

4. **签名**：
   - 每次更新本文档，**必须**同步更新顶部"最后核验日期"与"事实来源"两行
   - 顶部"事实来源"列出的文件清单要与末尾"最后核验"段落引用的文件清单**一一对应**，不出现"列 8 份但只对应 7 份"的失配
   - commit message 模板：`docs(ai-context): refresh to YYYY-MM-DD — <changes summary>`

---

_最后核验：2026-07-12。事实来源以本文档顶部"事实来源"清单为准（顶部清单与末尾引用一一对应）。_
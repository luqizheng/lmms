# AI Context — coder-micro-services

> **文档性质**：状态快照 + AI 协作入口（**不是**架构教程或历史流水账）。
> AI 协作者在本仓库工作前**必须先读**本文件；它把当前事实、漂移点、未完成项收口到一处。
>
> **最后核验日期**：2026-08-19
>
> **事实来源**（已核对）：
> - `docs-site/ai-context.md`（本文旧版本）
> - `pre-v1-todo.md`（已全部 P0/P1/P2 勾选，commit `b991ac5`）
> - `done.md`（本轮整改归档，commit `b8e1679`）
> - 根 `README.md`、`.gitlab-ci.yml`、`infrastructure/README.md`
> - `services/*/Cargo.toml`（10 个 Rust service crate，其中包含 2 个独立 publisher crate）
> - `infrastructure/docker-compose-{infra,services,observability}.yml`（infra 含 `mysql-replica`，observability 含 `alertmanager`）
> - `infrastructure/config/prometheus/{prometheus,alerts}.yml`、`infrastructure/config/alertmanager/alertmanager.yml`
> - `infrastructure/config/grafana/dashboards/{gateway,iam-service,notification-service,audit-service,config-service,file-service,prue_backend}.json`
> - `infrastructure/mysql-replica/{Dockerfile,replica-entrypoint.sh}`
> - `services/config-service/src/routes/config.rs`（reveal policy 调用点）
> - `services/notification-service/src/bin/outbox_publisher.rs`（ntf-outbox-publisher binary）
> - `services/iam-outbox-publisher/AGENTS.md`（`/metrics` 显式豁免；用 process exporter）
> - `docs-site/guides/migrations-state.md`、`docs-site/runbooks/{secret-rotation,backup,mysql-replication,log-aggregation}.md`（log-aggregation 新增于 2026-08-18）
> - `docs-site/decisions/{ADR-003-multi-tenancy-architecture,ADR-004-release-engineering-architecture,ADR-005-shared-db-vs-per-service-db,ADR-006-gateway-long-term-strategy,CONVENTIONS-libs-crate-strategy,frontend-e2e-coverage-eval,test-case-migration-eval}.md`
> - `docs-site/{slo,perf-baseline}.md`、`docs-site/plans/k3s-decision-review.md`
> - `sdk/java/KNOWN-DRIFT.md`（12 处 SDK/OpenAPI 端点漂移）
> - `scripts/check-{java-sdk-drift,i18n-namespace,doc-links,service-standard,spec,db-naming,reconcile-todo-docs}.js`
> - `tests/load/baseline.js`（k6 基线脚本）
> - `docs/superpowers/plans/2026-06-25-overall-roadmap.md`
> - `docs/superpowers/specs/2026-07-23-{release-dist-template,setup-wizard,sdk-publishing}-design.md`（P1 文档基建追认，commit `28d0aead`）
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

2. **当前阶段**：12 周路线图（Phase 1-3）+ Phase 4（事件总线）+ Phase 5-A-2（registrator 替 Consul Connect）+ Phase 5-B（JWT 验签 + service-jwt）+ Phase 6-A（OTel + outbox 生产化）+ Phase 6-B（config-service dogfooding）+ Phase 8（多租户）全部 [完成]；Phase 9（Java 集成）[完成]；**Phase 10 与 Roadmap 2.0 仍有 [待推进] 项**。

---

## 1. 服务地图

### 1.1 Rust service crate（10 个）+ 同 crate 内 1 binary

`services/` 下 Rust crate 实际清单（已核对 Cargo.toml，2026-07-12）：

| Crate | 角色 | OTel feature | OTel env（compose） | 备注 |
|---|---|---|---|---|
| `api-gateway` | JWT 验签、Redis 黑名单、路由、限流 | 显式 `otlp` | 已设 `OTEL_EXPORTER_OTLP_ENDPOINT` | 唯一对外 HTTP 入口；容器 + 宿主机端口均 3000 |
| `token-service` | access/refresh/service-jwt 签发 | 显式 `otlp` | 已设 | `/tokens/.well-known/jwks.json` |
| `iam-service` | 用户 / 角色 / 权限 / 部门 / 租户；事件生产者 | 显式 `otlp` | 已设 | `iam_outbox_events` 表；`/iam/internal/*` 端点验 `aud="iam-service"`；2026-08 P0 修复：补开 `otlp` feature（此前 resolver=2 + 单独 `cargo build -p iam_service` 下 exporter 注册分支被编译掉），守门见 §8 第 5 条 |
| `iam-outbox-publisher`（独立 publisher crate） | 轮询 `iam_outbox_events` → RabbitMQ | 显式 `otlp`（`default-features = false` + `["otlp"]`） | 未设 | 独立 crate（Phase 6-A T2-fu3 拆出），仅依赖 sqlx + digua_common-event + digua_common-telemetry + serde_yaml；compose 复用 `iam-service` Dockerfile 切 CMD；无 HTTP / 无 openapi.yaml / 无 migrations / 无 tests / 无自有 Dockerfile |
| `notification-service` | 公告 CRUD + WebSocket 推送 + 本地 user membership 投影 | 显式 `otlp` | 已设 | 同 crate 内另含 `[[bin]] ntf-outbox-publisher`（轮询 `ntf_outbox_events` → RabbitMQ；OTel 未设 env）；订阅 `iam.user.*` / `iam.permission.changed` / `notification.announcement.published` |
| `audit-service` | 操作日志 + 登录日志；事件消费者 | 显式 `otlp` | 未设 | RabbitMQ consumer；envelope.trace_id → span parent |
| `config-service` | 命名空间、配置项、watch；6-B dogfooding 客户端 | 显式 `otlp` | 已设 | Phase 8.3-fu2 后引入 Principal 鉴权 + namespace ownership；`is_secret` 字段见 §4.1 |
| `file-service` | 文件元数据、预签名上传、下载 | 显式 `otlp` | 已设 | VARCHAR → BIGINT 迁移（V002）；存储层见 §4.1 |
| `prue_backend` | pure-admin 菜单配置 + 动态路由 | 显式 `otlp` | 未设 | 保留 SQLite + MySQL 双栈（spec `2026-07-08-prue-backend-sqlite-decision-design.md`），`scripts/check-prue-migrations-parity.sh` 守 parity |

> **OTel 口径（按代码 + compose 证据校准）**：
> - **可观察行为差异**：compose 内未设 endpoint 的进程运行时是 stdout-only，**不会出现在 Jaeger UI**；开发/本地直跑未设 endpoint 同理。
>

### 1.2 Java 业务服务（2 个）

| 服务 | 端口（容器 / 宿主机） | 技术栈 | 状态 |
|---|---|---|---|


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
> **端口以 docker compose 为准**（host 端口 → 容器内端口 = `:PORT`）：api-gateway 3000:3000、token 8082:8082、iam 9081:8081（mesh 残留，待统一 8081:8081）、notification 8083:8083、audit 8084:8084、prue 8087:8087、config 8085:8081、file 8086:8082。`cargo run` 本地默认与 compose 不同（token 4000、audit/prue 8080、iam 8081）——需带 compose env 才落到 808x。完整表见 [`docs-site/user-manual/11-reference.md` §11.5](user-manual/11-reference.md#115-服务端口速查phase-5-a-2-之后) 与 [`docs-site/user-manual/02-architecture.md` §2.5](user-manual/02-architecture.md#25-swagger-ui-覆盖)。

---

## 2. 关键事实速查（AI 5 分钟可读完）

- **架构**：浏览器 → `api-gateway:3000` → Rust 业务服务；服务间调用 `Authorization: Bearer <service-jwt>` 透传（无 mTLS / 无 SPIFFE）；事件总线走 RabbitMQ + transactional outbox。
- **JWT**：RS256；网关验签；下游用 `libs/digua_common-jwt::JWKSCache`；`Principal` extractor 取身份（`libs/jwt-ctx`）；**禁止读 `X-User-*` headers**。
- **多租户**：共享 MySQL + `tenant_id` 列；JWT `tid` claim + `tenant_id` 必填（缺则 401）；`cross_tenant: bool` 给 platform-admin；`scripts/check-tenant-scope.js` lint 守门。
- **契约层**：`contracts/error-codes.yaml` / `permissions.yaml` / `events.yaml` / `http-conventions.md` / `db-conventions.md` 单一真相源。`scripts/contracts-gen.js` 生成 `libs/digua_common-error/src/codes.rs`、`libs/digua_authz/src/perms.rs`；**禁止手改生成文件**。

---

## 3. 阶段矩阵（Phase 1-10 + Roadmap 2.0）

> 数据来源：TODO.md 全部勾选状态 + 旧 ai-context.md + `2026-06-25-overall-roadmap.md`。

| Phase | 主题 | 状态 | 关键证据 |
|---|---|---|---|
| 1 | 服务规范 sweep + prue 并发 + active routes 净化 | [完成] | ADR-002 + 8-item baseline + 3 CI gates；spec `2026-06-25-prue-backend-concurrency-design.md` |
| 2 | Swagger 全覆盖 / OpenAPI auto-dump / skills / L4 triggers / 前端 P1 | [完成] | 8/8 Swagger 覆盖；`node scripts/dump-openapi-specs.js`；`.agents/skills/{coder-add-route,coder-migrate-db,coder-new-service}`；`.trae/rules/project_rules.md` |
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
| 10 | 小问题清单（收尾） | [部分完成] | 详见 §4.1 |
| R2.0 | TLS termination / RabbitMQ/MySQL/Redis TLS | [待推进] | 阻塞：1.0 落地 + 至少 1 个外部租户实际接入并提出合规 |

---

## 4. 当前未完成项

### 4.1 Phase 10 [部分完成]（TODO.md 现存 15 项中按主题归类）

**健康检查**：
- migration-state 端点（`/<svc>/internal/migrations/state`）：**已核对 `docs-site/guides/migrations-state.md` 为 v1，Phase 10 G3 落地**，列 7 个 Rust 服务（iam V001-V005 / notification V001-V003 / token / audit / config V001-V002 / file V001-V002 / prue_backend V001 × 2 dialects）。三项均已闭合（2026-08-19）：
  - ① 集成测试：CI job `e2e_migration_state_test`（`.gitlab-ci.yml`）+ `scripts/check-migration-state.js` 本地脚本（7 服务 × HTTP 200 + dialect + applied ≥ min + in_sync + current_version 非空）
  - ② TODO.md 已勾选（commit 在 `feat/reconcile-todo-docs` 分支 `57ae894`，与 `scripts/reconcile-todo-docs.js` 同步）
  - ③ 各服务 `AGENTS.md` 端点条目 + guide 引用由 `docs-site/guides/token-service-overview.md:265` 反链引导
- 漂移守护：`scripts/reconcile-todo-docs.js`（R1 = 父行"待 XXX"但子项全 [x]；R2 = guide 状态行已落地但 TODO [ ]）；CI `reconcile_todo_docs_check` job，TODO.md / docs-site/** 改动时触发，1 条 HARD 即 fail MR。

- 外部 `config/application.yml` 死代码：原 `${DATABASE_URL}` / `${DB_USER}` / `${DB_PASSWORD}` 占位符（Spring 实际读 `SPRING_DATASOURCE_*`）已移除（TODO.md Phase 10 第 456 行 `[x]` 勾选；当前 `examples/java8-webapi/demo-webapp/src/main/resources/application.yml` 不再含该段）。

**config-service 缺口**（实际代码复核已完成，2026-08-19）：
- `services/config-service/src/routes/config.rs` 已实现 `apply_reveal_policy` + 常量 `REVEAL_PERM = "CFG_CONFIG_REVEAL"`，在 **5 个 handler**（`create / query / get / update / export`）调用点对 `is_secret=true` 项做掩码，`principal.has_permission(REVEAL_PERM)` 决定是否透明；**不调用**点：`delete_config_item`（无 value 返回）、`import_config`（强制 `is_secret=false`）。代码侧 reveal policy 已落地。三项独立核对结果（2026-08-19）：
  - ① **已闭合**：`contracts/permissions.yaml:70` 注册（`description: 查看敏感配置项明文`）；iam 绑定走 `services/iam-service/migrations/mysql/V005__platform_admin_role.sql` + `V004__tenant_business_tables.sql` 的 `INSERT IGNORE ... SELECT id FROM iam_permissions WHERE status='enabled'` 机制（V005 L10-11 / V004 L46 全选 enabled），因此 `platform_admin` 与 `tenant_admin` 自动持有 `CFG_CONFIG_REVEAL`。后续若 `disable` 该 perm，admin 自动丢失——依赖链清晰。
  - ② **已闭合（2026-08-19）**：新增 `services/config-service/tests/reveal_handler_e2e_test.rs`，仿 `tenant_isolation_test.rs` 范式用 `tower::ServiceExt::oneshot` + testcontainers MySQL 走真实 axum router（**不**走内部函数），覆盖 5 handler × {无 REVEAL → MASK、有 REVEAL → 明文} = 10 case，**外加** 401 反向防御与 non_secret 不受影响 2 个 boundary case。in-source 函数级 6 case 仍保留在 `src/routes/config.rs` `#[cfg(test)] mod tests`，baseline 4 case 在 `tests/reveal_policy_test.rs`。三层覆盖（内部函数 / 路由 wire-level / 反向）从 §4.1 之前遗留漂移转为 **[完成]**。
  - ③ **已闭合**：`services/config-service/AGENTS.md` "敏感字段掩码（reveal policy，已落地）" 段落同步了 5 handler 调用点 + 2 不应用点 + 权限码位置 + 默认绑定机制（V005/V004 全选 enabled）+ 测试入口（in-source / baseline / e2e 三层）。

**file-service 存储层缺口**：
- 实际仅实现 `LocalStorage`（实现 `storage/object_store::ObjectStore` trait）；`S3Storage` / `RustfsStorage` / 其他 object store adapter **尚未实现**。生产部署若需 S3 兼容对象存储，需补对应 implementation 并在 `services/file-service/src/storage/{s3,rustfs}.rs` 实现 trait + 注册到 `storage/mod.rs`。

**评估项**：
- 评估自研 router 替换为 Envoy / Pingora 的成本/收益 — **ADR-006 已落**（`docs-site/decisions/ADR-006-gateway-long-term-strategy.md`，2.0 评估 Envoy vs Pingora 触发条件明确）。

**前端遗留**：
- ~~i18n 三个 key 缺失~~ → **✅ 已修复（commit `c2d23b4`）**：实际扫描发现 10 keys（8 from buttons + 2 from login 含 1 个三副本）调用 `menus.*` 但定义在错误 namespace。迁移后 zh-CN / en 各 586 keys 完美对称，0 个 menus.* 调用 fallback。完整扫描见 §4.2 commit `c2d23b4`。

**指标**：
- `iam_outbox_archived_total` + `iam_outbox_purged_total` IntCounter [**豁免**] —— `iam-outbox-publisher/AGENTS.md` 显式声明 `/metrics` 豁免（无 HTTP；process exporter 抓 PID）。重新评估触发条件保留在 `done.md §P0-2 5 节`。
- Prometheus `/metrics` 覆盖完整：5 条 alert 规则（ServiceDown critical / HighErrorRate critical / HighLatency warning / OutboxBacklog warning / HighRateLimit warning），定义在 `infrastructure/config/prometheus/alerts.yml`。

### 4.2 pre-v1-todo.md 全部条目（2026-07-30 ✅，commit `b991ac5`）

> 来源：`pre-v1-todo.md`（架构评审整改清单）。本节是**状态快照**，不再列具体方案——具体方案见对应 ADR / 决策文档。

| 编号 | 主题 | 状态 | 落地证据 |
|---|---|---|---|
| P0-1 | 文档信任链断裂 | ✅ | commit `bf6ed13` + `scripts/check-doc-links.js` |
| P0-2 | CI/CD 补齐 | ✅ | commit `e24a3f4`：9 服务 build + clippy/fmt/eslint + audit/deny + 滚动 deploy |
| P0-3 | 明文密码泄露 | ✅ | commit `68ce9d0`：从 .gitlab-ci.yml / README 移除 + gitleaks pre-commit |
| P0-4 | 备份 Runbook | ✅ | commit `272c071`：`docs-site/runbooks/backup.md` |
| P0-4 剩余 | MySQL 主从 | ✅ | commit `e905828`：GTID-based async replication + `mysql-replication.md` |
| P1-1 | 观测栈闭环 | ✅ | Alertmanager + 5 alert + 6 服务 dashboard + `slo.md`（commit `e24a3f4`） |
| P1-2 | DB 隔离 ADR | ✅ | commit `e24a3f4`：`ADR-005-shared-db-vs-per-service-db.md` |
| P1-3 | 网关策略 ADR | ✅ | commit `e24a3f4`：`ADR-006-gateway-long-term-strategy.md` |
| P1-4 | K8s 决策重审 | ✅ | commit `e24a3f4`：`k3s-decision-review.md` |
| P1-5 | 负载摸底 | ✅ | commit `e24a3f4` + `d1212a7`：`perf-baseline.md` + `baseline.js` |
| P2-1 | crate 拆分策略 | ✅ | `CONVENTIONS-libs-crate-strategy.md` |
| P2-2 | SDK 重叠 | ⬜ 推迟 2.0 | 见 ADR-006 / SDK plan |
| P2-3 | e2e 覆盖评估 | ✅ | `frontend-e2e-coverage-eval.md` |
| P2-4 | outbox 指标 | ⬜ AGENTS 豁免 | `iam-outbox-publisher/AGENTS.md` |
| P2-5 | test-case 迁移评估 | ✅ | `test-case-migration-eval.md` |
| P2-6 | Java SDK drift | ✅ | `scripts/check-java-sdk-drift.js` + `KNOWN-DRIFT.md`（12 处 drift） |
| P2-7 | i18n namespace | ✅ | commit `c2d23b4`：10 keys 迁移、zh/en 各 586 对称 |
| P2-8 | 集中式日志（Loki + Promtail） | ✅ | commit `f04fa8e` (T1 access log + trace_id) + `112b8a7` (iam-service otlp) + `434c6b7` (ADR-007) + `2026-08-18` runbook/e2e drill/log-format CI gate |


### 4.3 Onboarding / 运维 [待推进]

来自 `docs/onboarding/` 系列 TODO：
- 内网仓库凭证申请流程（Docker Registry / Maven）—— 待运维补充
- 生产数据迁移 SOP（备份 / 回滚 / 灰度验证）—— 待运维补充
- 集中式日志收集方案（ELK / Loki / 阿里云 SLS）—— ✅ **[完成]**（2026-08-18）：Loki 单 binary staging + Promtail docker_sd 已落地；commit `f04fa8e` (T1 access log 带 trace_id) + `112b8a7` (iam-service otlp feature) + `434c6b7` (ADR-007)；新增 `docs-site/runbooks/log-aggregation.md` 5 步 SOP、`scripts/e2e-log-traceability.js` drill、`scripts/check-log-format.sh` CI gate
- 求救渠道 / on-call 值班表 —— 待团队负责人补充
- commit message 规范（是否走 Conventional Commits / PR 标题 / issue 关联）—— 待规范人补充
- config-service 使用方式 —— 待熟悉 config-service 的开发者补充
- 前端登录态刷新机制细节 —— 待前端负责人确认

#### 4.3.1 多租户注意事项（来自 [ADR-003](../decisions/ADR-003-multi-tenancy-architecture.md)）

> 完整论证见 ADR-003；本节为 AI 协作者快速执行清单。

**1. JWT `tid` 必填**
- 所有 JWT（access / refresh / user / service）必带 `tid` claim，缺失硬拒 401
- `ct: true` claim 表示 platform_admin 跨租户能力（不替代 `tid`）
- Gateway 只检查 `tid` 存在性，**不解析** request body / path params 决定租户隔离 —— 跨租户过滤由下游 SQL `WHERE tenant_id = ?` 兜底

**2. 三层防线（任一挡住即不泄露）**
1. JWT `tid` claim（认证层）
2. `TenantScope` extractor（业务入口，`libs/jwt-ctx::scope`）
3. SQL `AND tenant_id = ?`（数据层，`QueryBuilder::and_tenant(&scope)`）

**3. 新表设计硬规则**
- **租户拥有资源**：`tenant_id BIGINT UNSIGNED NOT NULL DEFAULT 1` + 索引（例: `iam_users`, `iam_roles`, `iam_org_units`, `ntf_announcements`, `cfg_namespaces`, `file_files`, `audit_operation_logs`）
- **全局资源**：不加 `tenant_id`（例: `iam_permissions`, `iam_menus`, `prue_menu_configs`）
- **事件流**：`tenant_id BIGINT UNSIGNED NULL`（consumer 容错历史事件）
- **唯一键必须包含 `tenant_id`**（如 `UNIQUE KEY (tenant_id, name)`）
- **跨服务不建 FK**：仅 `iam_*` 表 FK → `iam_tenants(id)`；其他服务用裸列 + 索引

**4. handler 拿 `TenantScope` → service 强制 `and_tenant`**
- `libs/digua_common-db/src/scope.rs` 提供 `QueryBuilder::and_tenant(&scope)`：`Single(tid)` → 追加过滤；`CrossTenant` → no-op
- `scripts/check-tenant-scope.js` 启发式扫描：handler 拿了 `TenantScope` 但 SQL 无 `tenant_id` 时 exit 1 阻断 push
- 误报可加 `// check-tenant-scope: delegated` 豁免注释（沿用模式）

**5. Roles 拆分**
- `iam_permissions`：**全局**（权限码定义全平台统一）
- `iam_roles`：**租户内**（每租户可自定义 role → permission 映射）
- 新租户注册时同事务创建 per-tenant `tenant_admin` + 绑定全部全局权限 + 赋给注册用户
- `platform_admin` 是**全局角色**，签发时 JWT 带 `ct: true`

**变更时机提醒**：新增租户拥有业务表 → 必须遵循 §3 分类 + 加索引 + tenant-included unique key；新 handler → 必须 extractor `TenantScope` + service 层 `and_tenant`；新 JWT 签发路径 → 必须带 `tid`；新 event envelope → 必须带 `tenant_id`。

### 4.4 Roadmap 2.0 [待推进]

- R2.1 基础设施层 TLS（RabbitMQ AMQP / MySQL / Redis）
- R2.2 TLS termination（alb / nginx 终结）
- 阻塞条件统一：1.0 落地 + 至少 1 个外部租户实际接入并提出合规需求

#### 4.4.1 发行工程打包流程（来自 [ADR-004](../decisions/ADR-004-release-engineering-architecture.md)）

> 完整论证见 ADR-004；本节为 AI 协作者快速执行清单。关联 spec: [release-dist-template](../../docs/superpowers/specs/2026-07-23-release-dist-template-design.md) / [setup-wizard](../../docs/superpowers/specs/2026-07-23-setup-wizard-design.md) / [sdk-publishing](../../docs/superpowers/specs/2026-07-23-sdk-publishing-design.md)。

**1. 分发形态：v2「平台镜像 + 开发骨架」**
- 核心平台以 Docker 镜像交付，`services/` 目录为空供用户自建业务服务
- 单包 `coder-micro-services-<VERSION>-airgap.tar.gz` ≈ 10-15 GB（镜像） + MB 级（骨架）
- 包含：bin / compose / config / images / frontend / `.agents/`(skills 仅白名单 6 scaffold + rules) / contracts / scripts / Cargo.toml / docker-compose.yml / Makefile / .env.example
- **放弃"服务源码全打包"路径**（太重、更新不便）

**2. 打包命令（`release.sh` 已于 2026-08 迁 Node 删除；↓ 传入/示例均为 **no-v** 版本，git tag 才带 `v`）**
```bash
node release/dist-template/scripts/release.js 1.0.0
# 骨架 only（跳过 docker pull + save）：
EXPORT_IMAGES=0 node release/dist-template/scripts/release.js 2.0.0
# 或 pnpm run release:build（= release.js，无参取默认版本）；单测 pnpm run release:test
```
- 平台服务 + 基础镜像清单硬编码在 `release.js` 的 `IMAGES` 数组：**9 个 micro-server（api-gateway/config/file/token/iam/notification/audit/prue-admin-backend/frontend）+ `micro-server/mysql-replica` + 基础镜像**。新增服务需同时改 `release.js` 的 `IMAGES` + `compose/services.yml` + `openapi.yaml`。
- **git tag 带 `v`，发布镜像 tag 不带 v**：`build_images:tagged` 用 `${CI_COMMIT_TAG#v}` 剥 v，registry 里是 `micro-server/<svc>:<no-v>`。离线下发/下游升级引用/对账命令里的 tag 一律 no-v（`v1.5.0` 是 git tag，镜像 tag 是 `1.5.0`）。
- airgap 全包依赖 `mysql-replica` 与 `alertmanager`：前者 CI tag pipeline buildx 单独推送并在离线包导出（主 compose 引用），后者走公网镜像代理完整地址（内网 registry 未同步）。离线缺镜像先查 `IMAGES` 是否漏导。

**3. 4 类 mirror registry（全部可留空走官方源）**
| 类别 | 官方源 | 替代 |
|---|---|---|
| Docker 基础镜像 | Docker Hub | Nexus / Harbor（`baseRegistry`） |
| Docker builder 镜像 | 官方 | 内网 registry（默认 `zhcoder-docker-registry.com:8000`） |
| Cargo crates | crates.io | rsproxy / kellnr 私有 registry |
| Maven | Maven Central | Nexus / Reposilite（`token` / `userpass` 认证） |
| npm | npmjs | 镜像仓库 |

**4. SDK 发布策略**
| SDK | 目标仓库 | 认证 |
|---|---|---|
| Rust libs（`libs/`）+ SDK（`sdk/rust/`） | 私有 kellnr | `.cargo/credentials.toml` 中 token |
| Java SDK（`sdk/java/`） | 私有 Reposilite（`192.168.2.12:18181`） | `.mvn/settings.xml` 中 servers |
| **.NET SDK** | ❌ **已废弃**（7 月清理） | — |

- Java SDK 0.3.0 起独立版本号，与平台版本解耦
- Rust SDK 版本策略未固化（ADR-004 已知未做）

**5. Setup wizard（零信任本地化配置）**
- 首次 clone 后跑一次：`node scripts/setup-wizard.js`，支持 `--non-interactive` / `--defaults` / `--show` / `--dry-run`
- 全部输出文件**不入 git**（个人/团队/CI 隔离）
- 同步部分选项到 `.gitlab-ci.yml` 的 `CODER_*` 变量
- ⚠️ `.cargo/config.toml` 被 wizard 覆盖式重写 → 用户手改的其他配置会丢（保留手工改 configs 前**先备份**）

**变更时机提醒**：新服务发布 → 改 `release.js` 的 `IMAGES` + `compose/services.yml` + `openapi.yaml`；新依赖源 → `setup-wizard.js` 加交互 + 生成对应配置；SDK 破坏性变更 → 升 major 版本 + `sdk/README.md` 记录 migration guide；v2 骨架结构调整 → 改 `release.js` 复制清单 + `release/dist-template/README.md`。

> **下游技能白名单**：发布产物 `.agents/skills/` 仅入 `DOWNSTREAM_SKILL_WHITELIST`（`release.js`）的 6 个 scaffold（Java/Rust × new-service / add-route / migrate-db）；仓库其余 repo-local skill 与`.trae/skills` 不随包下发，dist 另写 `.agents/README.downstream.md`。新增想随包分发的 skill → 在 `DOWNSTREAM_SKILL_WHITELIST` 追加条目后重新发包（勿在本仓把全部 skill 下沉）。

---

## 5. CI 与本地差距（2026-08 重写）

来源：`.gitlab-ci.yml` 已核对全文（含 2026-08 的 SSH-deployed 重构）。`stages` 现在 6 段：`test / build / deploy / e2e / publish / release`。

### 5.1 CI 当前行为

| 阶段 | Job | 触发 | 实际跑什么 | 状态 |
|---|---|---|---|---|
| `test` | `rust_test` | 所有 pipeline | `cargo test --workspace --lib` | [正常] |
| `test` | `service_standard_check` | 所有 pipeline | `node scripts/check-service-standard.js` | [正常] |
| `test` | `spec_check` | 所有 pipeline | `node scripts/check-spec.js` | [正常] |
| `test` | `contracts_check` | 所有 pipeline | `node scripts/contracts-gen.js --check` | [正常] |
| `test` | `spec_generate` | 所有 pipeline | `node scripts/dump-openapi-specs.js` + `git diff --exit-code` | [正常] 守 openapi drift |
| `test` | `doc_links_check` | 所有 pipeline | `node scripts/check-doc-links.js` | [正常] |
| `test` | `cargo_clippy` / `cargo_fmt_check` / `cargo_audit` / `cargo_deny` / `frontend_lint` / `java_sdk_test` / `java_sdk_drift_check` / `i18n_namespace_check` | 条件 | 各自脚本 | [正常] |
| `deploy` | `deploy` | `only: main` | **SSH 到 `STAGING_HOST` + 远程 `deploy-remote.sh` 滚动重启 + 端点验证** | [正常] 不再依赖专属 runner；走 `tags: [docker]` + SSH key |
| `deploy` | `deploy_smoke` | `needs: deploy` | `curl $STAGING_BASE_URL/health = {"status":"ok"}` | [正常] `allow_failure: true` |
| `e2e` | `e2e_api_test` | `needs: deploy` | 走 `STAGING_BASE_URL=http://192.168.2.127:8080`（frontend → api-gateway） | [正常] 不再依赖 3000 暴露 |
| `e2e` | `e2e_ui_test` | 同上 | Playwright 走 frontend:8080 | [正常] |
| `e2e` | `e2e_migration_state_test` | `needs: deploy` | 通过 SSH-tunnel（19xxx 端口）直连业务后端 | [正常] 7 端口 × service-jwt |
| `build` | `build_<svc>`（9 个）+ `build_frontend` | `changes:` 条件 | `.rust_svc_build` 模板 + `docker push` | [正常] |
| `publish` / `release` | `sdk_publish` / `rust_publish` / `release:dist` | tag | 内网 Maven / kellnr / 离线分发包 | [正常] |

### 5.2 凭据管理（合规且对齐，2026-08 更新）

`.gitlab-ci.yml` 中所有凭据**已迁移至 GitLab CI/CD Variables**（masked + protected），yml 仅引用 `${VAR_NAME}`：
- `SSH_PRIVATE_KEY` —— **必填**，部署机 ed25519 私钥
- `SSH_KNOWN_HOSTS` —— 可选，推荐填
- `IAM_DEV_BT` / `TOKEN_DEV_BT` / 等 7 个 service-jwt bootstrap token —— masked GitLab 注入
- `E2E_DB_PASSWORD` —— masked GitLab 注入
- `KELLNR_TOKEN` / `MAVEN_USERNAME` / `MAVEN_PASSWORD` / `NPM_TOKEN` —— Rust/SDK 发布用
- `STAGING_HOST` / `STAGING_DEPLOY_USER` / `STAGING_PROJECT_DIR` / `STAGING_SSH_PORT` / `STAGING_BASE_URL` —— 有 default，`${VAR:-default}` 形式
- 旧死代码：`DEPLOY_SERVER` / `DEPLOY_USER` / `DEPLOY_PASSWORD` —— **已在 2026-08 重构中删除**

**完整回填清单**：见 `infrastructure/scripts/CI_VARIABLES.md`（按表填即可，5 分钟）。

### 5.3 部署模型（prod overlay，2026-08 落地）

按 `docs/superpowers/specs/2026-07-06-prod-compose-overlay-design.md` 落地：

- 唯一 host 公开端口：`frontend:8080`（仅此而已）
- `api-gateway:3000` / 业务后端 8081-8087 / `mysql` / `redis` / `rabbitmq` / `consul` host port 全部清掉（`!override []`）
- observability 端口（Prom 9090 / Grafana 3001 / Jaeger 16686 / Otel 4317-4318 / Alertmanager 9093）保留（ops 直连）
- 全部长跑组件 `restart: unless-stopped`
- 起停入口：`infrastructure/scripts/{start-prod.sh, stop-prod.sh}`（fail-fast compose v2.20+ + env 分层加载 + 关键 vars 必填校验）
- 远程部署编排：`infrastructure/scripts/deploy-remote.sh`（preflight + pull + rolling restart + healthcheck 门禁 + frontend:8080/health 端点 + image prune）

**门禁**：
- `.gitlab-ci.yml` `deploy` job SSH 用 `StrictHostKeyChecking=accept-new` + `ExitOnForwardFailure=yes`
- `deploy-remote.sh` exit code 10/20/30/31/32 区分 preflight / pull / 重启 / health / 端点验证 fail
- 旧 `micro-service-runner` tag 假设已替换为通用 `[docker]` executor + SSH

**门禁**：`.pre-commit-config.yaml` 已加 gitleaks hook（commit `68ce9d0`）防止再次明文提交。

### 5.4 CI / deploy 修补基线（2026-08-22 … 08-31，HEAD `69f6ecda`）

迭代加固记录（多为 `fix:` 小步，合并进 main、经常互为 revert/改道），用于理解当前 `.gitlab-ci.yml` 形状而非逐条背诵：

- **发布管道是三轨**：日常 `main` / MR pipeline 只跑 test+build+deploy:e2e；**tag（release）pipeline** 另跑 `sdk_publish` / `rust_publish` / `maven_publish` / `release:dist` + `build_images:tagged`（剥 v 推镜像）。tag e2e jobs 设为**非阻塞**（`allow_failure`，release 不被回归阻塞）；`.pre-staging` 链已废弃，只留 continuous `deploy:staging`。
- **staging 冷启动 health gate 用轮询**（commit `075b8af0`）：`api-gateway:3000/health` 每 10s 一次、最多 30 次（300s）；全空卷冷启动时 MySQL 初始化 + migration/seed 需数分钟，单次 curl 会过早超时。deploy + e2e:staging 依赖端口是 **api-gateway:3000（不是 8080）**。
- **runner 是 shell / `[docker]` executor + SSH 部署**（root-owned workspace）：多次根因是 `sudo docker` 产物 root-owned → compose/buildx 后续 clean 失败。当前已自愈：`cleanup:buildx` 重新 `chown` workspace、build 后 `chown` 回文件、`release-staging` 重映射挂载避开 root-owned 残留。
- **tag 发布类 job（shell executor）**：需要在 maven 镜像里补装 pnpm/Node；publish npm 走 Verdaccio basic auth（user:pass，非 authToken）；`deploy:staging` 的 .env 首启从 `.env.example` 兜底、health gate 探针、ssh 远端用 `cat` 避开 bash 引号嵌套等已逐条修平。
- **staging 相关修复点**：`frontend` 加 `extra_hosts pm=127.0.0.1` 让 nginx 启动过；大 swap 重启前先 warm up 基础服务（修 DNS）；`staging-${SHA}` swap 探活探针边界修正。观察性 runbook 增补见 commit `7ca144d5`（§6=staging 第三方基础镜像清单）与 `9d54beba`（§8.4 swap-noop + health gate 0ms refused，写在 `docs-site/runbooks/troubleshooting.md`）。
- **tag/release 产物规范**：`dist-template` 的 `IMAGE_TAG` 统一 **no-v**（git tag `v1.5.0` ≠ 镜像 tag `1.5.0`）；tag pipeline buildx 单独构建并推送 `micro-server/mysql-replica`（主 compose 引用、离线包需导出）；`mysql-replica` / `alertmanager` 镜像对 tag pipeline 的构建/推送是新增职责。
- **门禁**：缺失/漂移时对照 §5.1 现行 gate 脚本（`*.js`，勿再依赖旧 `.sh`）。
- 运维侧 CD/canary/发布机制文档见 `docs-site/runbooks/release-tag-pipeline.md` 等。

---

## 6. 可观测性现状（2026-08-18 重写）

| 组件 | 当前实际 | 状态 |
|---|---|---|
| Prometheus | `infrastructure/config/prometheus/prometheus.yml`（consul SD + 自抓 + otel-collector）；**alerting 块启用** + `rule_files: ['alerts.yml']` | [正常] |
| Alertmanager | **✅ 新增**（commit `e24a3f4`）：镜像**用公网代理 `swr.cn-north-4.myhuaweicloud.com/ddn-k8s/quay.io/prometheus/alertmanager:v0.27.0`**（内网 registry 未同步，勿改回内网路径，见 commit `a77fba3c`/`b6547112`），端口 9093，volume `alertmanager_data`；配置 `infrastructure/config/alertmanager/alertmanager.yml`（route + receivers + inhibit_rules）。离线 airgap 须导出此完整地址 | [正常] |
| Alert rules | **✅ 新增**（`infrastructure/config/prometheus/alerts.yml`）5 条：ServiceDown critical / HighErrorRate critical / HighLatency warning / OutboxBacklog warning / HighRateLimit warning | [正常] |
| Grafana dashboards | **✅ 7 个**（commit `e24a3f4`，旧版本只 1 个 gateway）：`gateway.json` + `iam-service.json` + `notification-service.json` + `audit-service.json` + `config-service.json` + `file-service.json` + `prue_backend.json`。每个含 `service` template var + 4 panel（Request Rate / Latency p50/p95/p99 / Error Rate / Health） | [正常] |
| Grafana provisioning | `datasources/prometheus.yml` + `dashboards/dashboards.yml`（watch `/etc/grafana/dashboards` 自动加载） | [正常] |
| **Loki** | **✅ 新增**（2026-08-18，Phase 6-A T2）：`infrastructure/config/loki/loki-staging.yaml`（单 binary + 本地 filesystem，**14 天 retention**，无 S3 backend）；容器端口 3100；labels 仅 `level / service / env`（低基数）；trace_id / tenant_id / user_id 走 JSON metadata（ADR-007 D3） | [正常] |
| **Promtail** | **✅ 新增**（2026-08-18）：`infrastructure/config/loki/promtail-staging.yaml`；`docker_sd_configs` 5s 周期，过滤 `logging=loki` container label（ADR-007 D2）；pipeline `cri: {}` + `json:` 抽出 trace_id / tenant_id / user_id（commit `f04fa8e` 注入） | [正常] |
| SLO 文档 | **✅ 新增** `docs-site/slo.md`：8 服务 SLO 表 + SLI 公式 + 错误预算策略 + 告警规则对应 | [正常] |
| Performance baseline | **✅ 新增** `docs-site/perf-baseline.md`：S1 health check 已实测（RPS 91.2 / p50 105.5ms / 0 errors / gw CPU 10.44%）；S2-S7 待 k6 安装后补全 | [正常] |
 | runbooks | **✅ 4 份**（旧 3 份 + log-aggregation，2026-08-18）：`secret-rotation.md` + `backup.md`（P0-4 覆盖 MySQL/Redis/RabbitMQ/Consul 4 组件手动 SOP）+ `mysql-replication.md`（P0-4 剩余，主从 GTID 复制 + Failover）+ **`log-aggregation.md`**（Phase 6-A T2 Loki/Promtail 5 步故障排查 SOP） | [正常] |
 | OTel 端到端 trace | 10 个 crate 全部显式开 `otlp` feature（2026-08 P0 修复 iam-service：补 `digua_common-telemetry` + `-axum` 的 `features = ["otlp"]`，删冗余直接依赖 `opentelemetry`/`tracing-opentelemetry`）；compose 只对 6 个 Rust HTTP 服务 + 2 个 Java 服务设置 `OTEL_EXPORTER_OTLP_ENDPOINT` | [正常] 区分"代码接入"与"compose 实际导出配置"；详见 §1.1 表格 |


---

## 7. 文档与架构冲突（2026-07-30 重写）

### 7.1 ~~两个 Spring Boot 3 Java 服务并存，功能重叠、定位不清~~ → [已迁移]

PM Platform 已于 2026-07-19 移除（commit 在 git 历史）。仅保留 `pm-platform`（`/pm` 路由，8088 端口，已 deprecated）。Java SDK 继续作为 consumer-only 客户端存在（详见 §1.2）。

### 7.2 ~~OTel 覆盖 vs compose env 不一致~~ → [已收敛]

旧 ai-context §7 第 11 条列的 audit/config/file/prue_backend OTel "—" 已全部纠正——它们确实开了 `digua_common-telemetry` 的 `otlp` feature。**剩余不确定项已清零（2026-08 P0）**：iam-service 曾漏开 feature（直接依赖 `opentelemetry` / `tracing-opentelemetry` 但无使用点，exporter 注册分支被 resolver=2 + 单独构建编译掉），已补 `features = ["otlp"]` 并删冗余依赖；守门见 §1.1 表格 + §8 关键约定第 5 条。

### 7.3 ~~部署 compose 路径漂移~~ → [已修复]

`.gitlab-ci.yml` `deploy` job **现在用 `sudo /bin/docker-compose -f infrastructure/docker-compose.yml`**（commit `e24a3f4`）。与本地 dev 流程一致。同时改为滚动更新：11 个业务服务按依赖顺序逐个 `up -d --no-deps` + 60s healthcheck 门禁；基础设施（mysql/redis/consul/rabbitmq）全程不 down。

### 7.4 ~~凭据明文在 CI yaml~~ → [已修复]

P0-3 处理（commit `68ce9d0`）：所有 `DEPLOY_PASSWORD` / `E2E_DB_PASSWORD` / 7 个服务 bootstrap token 已迁 GitLab CI/CD Variables（masked + protected），yml 仅引用。gitleaks pre-commit hook 守门。

### 7.5 ~~Java SDK 端点漂移~~ → [已检测 + 已知清单]

`scripts/check-java-sdk-drift.js`（commit `f4374f5`）检测 SDK 调用路径 vs OpenAPI 实际路径。**已知 12 处硬失败**，列在 `sdk/java/KNOWN-DRIFT.md`：audit 3 / config 2 / file 3 / notification 4。CI `java_sdk_drift_check` job `allow_failure: true`（脚本报告但不阻断）。**修复责任**：逐项评估服务侧补端点 vs SDK 改路径（详见 KNOWN-DRIFT.md 修复方向 A/B/C）。

---

## 8. 关键约定与坑点（仍准确，保留）

> 以下是经实测仍生效的约定，新 AI 协作者请遵守。

1. **`utoipa::OpenApi::to_yaml()` 需要 `yaml` feature**：`Cargo.toml` 必须 `utoipa = { version = "5", features = ["axum_extras", "chrono", "yaml"] }`。缺了 `ApiDoc::openapi().to_yaml()` 编译不过。

2. **本地 `cargo` 编译可能因 `cc` 链接器缺失失败**：用 `node scripts/run-tests-docker.js <svc>` 或在 `zhcoder-docker-registry.com:8000/builder/rust:1.96-slim-musl` 容器内构建。Windows 上另有一条主机侧路径：`node scripts/run-tests-host.js <svc>`（在 WSL2 发行版里跑 cargo，数据库仍连 Docker 里的共享 MySQL/Redis）；Windows 原生 MSVC 走不通——`ring` / `aws-lc-sys` 的 C 依赖会报 `D8050 无法加载 c1.dll`。

3. **`api-gateway` 黑名单测试是历史遗留失败**：它要求宿主机 Redis 在 `127.0.0.1:6379`；与任何在途改动无关。Docker 跑（用测试 Redis）可通过。

4. **手写 `openapi.yaml` 已废止**：全部 10 份由 utoipa / springdoc 自动生成。改 spec 改代码（`#[utoipa::path]` / `@Operation`），再跑 `node scripts/dump-openapi-specs.js`。**不要直接编辑 yaml**。

5. **OTel traces 靠 env + feature 双开**（口径已按 Cargo.toml + compose 实际校准）：
   - 全部 10 个 Rust crate 已显式开 `otlp` feature（2026-08 P0：iam-service 曾漏开——workspace `resolver = "2"` 下 Dockerfile 单独 `cargo build -p iam_service` 时依赖闭包内 feature 只由服务自身声明决定，`digua_common-telemetry` / `-axum` 未带 `features = ["otlp"]` 导致 `init_tracing` 的 `#[cfg(feature = "otlp")]` exporter 注册分支被整体编译掉，span 永远到不了 Jaeger；已修复并删除冗余直接依赖 `opentelemetry` / `tracing-opentelemetry`）。
   - 任意一端（feature 或 env）缺失 → 进程不导出 span 到 OTel collector。判定方法：启动后查日志是否出现 `OTel tracing enabled → <endpoint>` 行；或在 Jaeger UI 搜不到对应 service / binary。
   - **守门**：`scripts/check-service-standard.js` 强制每个 Rust 服务声明 `digua_common_telemetry` 时显式带 `otlp` feature（豁免：AGENTS.md `## 豁免` 写 `OTel otlp feature`）；`libs/common-telemetry` 内 `otlp_exporter_registration_tests`（`#[cfg(feature = "otlp")]`，CI `cargo test --workspace --lib` 下 feature union 启用）验证 `init_tracing` 设 endpoint 后真实注册 OTLP pipeline。

6. **迁移是 forward-only**：不要编辑已发布迁移；新增 `V{N+1}__*.sql`。`scripts/check-prue-migrations-parity.sh` 守 prue_backend MySQL/SQLite parity。

7. **JWT 上下文只用 `Authorization` header**（`.trae/rules/auth-rules.md`）。任何地方**禁止**读 `X-User-*` headers。用 `libs/jwt-ctx::Principal` extractor。

8. **`check-spec.js` 报缺少 `summary` 是 warning 不是 error**；exit 0。utoipa 默认不生成 summary。

9. **`scripts/contracts-gen.js --check` 会重新生成 `libs/digua_common-error/src/codes.rs`**：不要直接编辑该文件。改 `contracts/error-codes.yaml`，再重跑。

10. **`notification/ws` 鉴权**：网关验 JWT 并注入用户身份；浏览器在 WS upgrade 时自动带 `Authorization`。**不要把 token 放 URL 里**。

11. **网关限流返回 429 + `Retry-After`**：查 `infrastructure/config/api-gateway/rate-limits.toml` 看按路由覆盖；默认 100 rps / 200 burst；`/v1/iam/auth/login` 5 rps / 10 burst。Phase 10 起 gateway 限流统一走 Redis（commit `ddf44a0`），淘汰 in-memory 实现。

12. **服务侧限流经 `digua_common-ratelimit` opt-in**：Redis 滑动窗口、跨副本共享、Redis 出错时 fail-open。模式见 `services/notification-service/src/rest.rs`。

13. **灰度通过 Consul `canary` tag**：consul tag + Nginx `split_clients` 接 ~10% 流量；用 `?canary=1` 或 `X-Canary: true` 强制路由。详见 `docs-site/guides/canary-rollout.md`。

14. **E2E Smoke 子集用 `E2E_SMOKE=1`**：只跑带 `smoke: true` 的用例，~2 分钟；CI 跑全量。详见 `docs-site/guides/testing.md`。

15. **i18n namespace 约定 + CI gate**（commit `c2d23b4` 后）：`frontend/locales/{zh-CN,en}.yaml` 中 `menus.X` 的 X 必须定义在 `menus:` namespace 下，**不能在 `buttons:` / `login:` / `search:` / `panel:`**（即使前缀同名也不允许——这是 yaml duplicate key 错误来源）。**`scripts/check-i18n-namespace.js`**（commit `f7c7237`，2026-07-30）已在 CI 守门：扫描 `frontend/src` 下所有 .vue / .ts 中的 `t('menus.X')` 调用，对照 yaml `menus:` namespace 校验；hard-fail 退出码 1。CI job `i18n_namespace_check`（`.gitlab-ci.yml`）按 `frontend/src/**/*.{vue,ts}` 或 `frontend/locales/**/*.yaml` 变更触发。

16. **事件总线 = RabbitMQ + transactional outbox**（Phase 4）。同事务写业务行 + `digua_common_event::MySqlOutboxRepo::enqueue(&mut tx, &Envelope::new(...))`；独立 `*-outbox-publisher` 进程轮询发布；消费者按 `event_id` 幂等。**`Envelope<serde_json::Value>` 仅用于消费**，publish 必须用强类型 Event。

17. **MySQL 复制对主库参数有硬要求**（commit `e905828` 后）：主库 `command` 必须含 `--server-id=1 --log-bin=mysql-bin --binlog-format=ROW --gtid-mode=ON --enforce-gtid-consistency=ON`，否则 replica `START SLAVE` 失败。从库用 GTID auto-positioning（`MASTER_AUTO_POSITION=1`），冷启动自动从主库克隆缺失事务，无需 mysqldump bootstrap。复制用户 `repl@'%'` 需在主库手工创建（见 `docs-site/runbooks/mysql-replication.md` §1）。

18. **CI 9 服务 build 用模板继承**：`.gitlab-ci.yml` 用 `.rust_svc_build` YAML anchor + `extends` + `changes:` 规则按变更触发，每个服务 build job 仅声明 `SVC_NAME` 变量。新增 Rust service 时：在 `services/<svc>/Dockerfile` 标准化后加 3 行（`build_<svc>`），无需复制 13 行模板。

19. **Alertmanager 与 Prometheus 告警对应**（commit `e24a3f4` 后）：`infrastructure/config/prometheus/alerts.yml` 5 条规则——ServiceDown（critical，对应可用性 SLO）/ HighErrorRate（critical，对应错误预算）/ HighLatency（warning，对应延迟 SLO）/ OutboxBacklog（warning，对应容量）/ HighRateLimit（warning，对应容量）。新增告警规则时：① yaml 加规则（PromQL + for + labels.severity）；② 重启 prometheus 容器（`docker compose restart prometheus`）。Alertmanager `route` 已按 severity 分发到 `default` / `critical` 两个 receiver。

---

## 9. 常用验证命令（push 前必跑）

```bash
# CI 门禁本地复刻
node scripts/check-service-standard.js      # services/** 服务规范
node scripts/check-spec.js                  # OpenAPI / 错误码 / 权限码
node scripts/contracts-gen.js --check       # contracts/** 生成代码未漂移
node scripts/check-db-naming.js             # 表前缀
node scripts/dump-openapi-specs.js               # 重新生成 openapi.yaml
git diff --exit-code -- 'services/*/openapi.yaml'  # drift 守门

# Rust 单元测试（docker 内）
node scripts/run-tests-docker.js audit-service config-service file-service prue_backend iam-service notification-service token-service api-gateway

# Rust 单元测试（主机侧；Windows 走 WSL2，Linux/macOS 走本机）
node scripts/run-tests-host.js audit-service config-service file-service prue_backend iam-service notification-service token-service api-gateway

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
| 调试请求链路（OTel / Jaeger） | `docs-site/guides/otel-tracing.md` |
| 灰度发布 | `docs-site/guides/canary-rollout.md` |
| 搭可观测性 | `docker compose -f infrastructure/docker-compose.yml up -d` + `docs-site/guides/observability.md` |
| 看 SLO / 错误预算 | `docs-site/slo.md` |
| 跑性能基线 | `tests/load/baseline.js` + `docs-site/perf-baseline.md` |
| 跑 / 写测试 | `docs-site/guides/testing.md` |
| 备份与恢复 | `docs-site/runbooks/backup.md` |
| MySQL 主从管理 | `docs-site/runbooks/mysql-replication.md` |
| 凭据轮换 SOP | `docs-site/runbooks/secret-rotation.md` |
| ADR（架构决策记录） | `docs-site/decisions/ADR-{001..008}*.md` + `CONVENTIONS-libs-crate-strategy.md` |
| 多租户注意事项快查 | §4.3.1（[ADR-003](../decisions/ADR-003-multi-tenancy-architecture.md)） |
| 发行工程打包流程快查 | §4.4.1（[ADR-004](../decisions/ADR-004-release-engineering-architecture.md) + 3 份 2026-07-23 spec） |
| K3s / 2.0 路线 | `docs-site/plans/k3s-decision-review.md` |
| E2E 覆盖率评估 | `docs-site/decisions/frontend-e2e-coverage-eval.md` |
| test-case 迁移评估 | `docs-site/decisions/test-case-migration-eval.md` |
| Java SDK 已知漂移 | `sdk/java/KNOWN-DRIFT.md` |
| 阶段总览 | `docs/superpowers/plans/2026-06-25-overall-roadmap.md` + `pre-v1-todo.md` + `done.md` |
| 设计 spec 目录 | `docs/superpowers/specs/`（含 2026-07-23 release-dist-template / setup-wizard / sdk-publishing 3 份发行工程追认） |
| 新人入门 | `docs-site/onboarding/README.md` |

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

_最后核验：2026-08-19。事实来源以本文档顶部"事实来源"清单为准（顶部清单与末尾引用一一对应）。本轮重大变更：

- **2026-08-19（合并 feat/centralized-logging + P1 收口）**：§4.1 第 1/2 条修正为"已闭合"（migration-state 三项 + application.yml 死代码移除）；新增 `scripts/reconcile-todo-docs.js` 漂移检测器（R1 父行 vs 子项 / R2 guide 状态行）+ CI `reconcile_todo_docs_check` job（`feat/reconcile-todo-docs` 分支 commits `ba3310b` / `57ae894` / `7f4f601`）。
- **2026-08-19（8 月 P1 文档基建收口）**：① 修复 §4.2 编号 duplicate bug（`### 4.2 Onboarding / 运维` → `### 4.3 Onboarding / 运维`，后续 4.3 → 4.4 顺延）；② 新增 §4.3.1 多租户注意事项（从 ADR-003 提取 5 条执行要点 + 与 §4.1 现有 tenant-scope 上下文衔接）；③ 新增 §4.4.1 发行工程打包流程（从 ADR-004 提取 5 条执行要点 + 链接 3 份 2026-07-23 spec 追认）；④ 顶部"事实来源"清单补 ADR-003/004 + 3 份 spec；⑤ §10 导航地图补 ADR-{001..008} + spec 目录入口 + §4.3.1/§4.4.1 跳板。ADR-003/004 本身已由 commit `28d0aead`（2026-07-26）落地，本轮仅做"事实化"收口。_
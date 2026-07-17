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
> - `services/config-service/src/routes/config.rs`（reveal policy 调用点）
> - `services/notification-service/src/bin/outbox_publisher.rs`（ntf-outbox-publisher binary）
> - `docs/guides/migrations-state.md`、`docs/runbooks/secret-rotation.md`
> - `docs/superpowers/plans/2026-06-25-overall-roadmap.md`
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
| `iam-service` | 用户 / 角色 / 权限 / 部门 / 租户；事件生产者 | **`common-telemetry` 未显式启用 `otlp`**；直接依赖 `opentelemetry` + `tracing-opentelemetry` 但 exporter 注册未独立核验 | 已设 | `iam_outbox_events` 表；`/iam/internal/*` 端点验 `aud="iam-service"`；OTel 实际导出行为待 §8 第 5 条说明 |
| `iam-outbox-publisher`（独立 publisher crate） | 轮询 `iam_outbox_events` → RabbitMQ | 显式 `otlp`（`default-features = false` + `["otlp"]`） | 未设 | 独立 crate（Phase 6-A T2-fu3 拆出），仅依赖 sqlx + common-event + common-telemetry + serde_yaml；compose 复用 `iam-service` Dockerfile 切 CMD；无 HTTP / 无 openapi.yaml / 无 migrations / 无 tests / 无自有 Dockerfile |
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
> **端口两套并存**：宿主机直接 `cargo run` 默认 6001-6007（见根 README「服务端口与依赖」）；compose 容器内 808x-9081（见 `infrastructure/README.md`「端口映射」）。两套互不干扰。

---

## 2. 关键事实速查（AI 5 分钟可读完）

- **架构**：浏览器 → `api-gateway:3000` → Rust 业务服务；服务间调用 `Authorization: Bearer <service-jwt>` 透传（无 mTLS / 无 SPIFFE）；事件总线走 RabbitMQ + transactional outbox。
- **JWT**：RS256；网关验签；下游用 `libs/common-jwt::JWKSCache`；`Principal` extractor 取身份（`libs/jwt-ctx`）；**禁止读 `X-User-*` headers**。
- **多租户**：共享 MySQL + `tenant_id` 列；JWT `tid` claim + `tenant_id` 必填（缺则 401）；`cross_tenant: bool` 给 platform-admin；`scripts/check-tenant-scope.js` lint 守门。
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
| 10 | 小问题清单（收尾） | [部分完成] | 详见 §4.1 |
| R2.0 | TLS termination / RabbitMQ/MySQL/Redis TLS | [待推进] | 阻塞：1.0 落地 + 至少 1 个外部租户实际接入并提出合规 |

---

## 4. 当前未完成项

### 4.1 Phase 10 [部分完成]（TODO.md 现存 15 项中按主题归类）

**健康检查**：
- migration-state 端点（`/<svc>/internal/migrations/state`）：**已核对 `docs/guides/migrations-state.md` 为 v1，Phase 10 G3 落地**，列 7 个 Rust 服务（iam V001-V005 / notification V001-V003 / token / audit / config V001-V002 / file V001-V002 / prue_backend V001 × 2 dialects）。`TODO.md` 仍把"集成测试 + 文档同步"列在 [ ] — 矛盾点：guide 已存在但 TODO 未勾选。**待核对**：① 各服务端点集成测试是否补；② `TODO.md` 该条目是否同步勾选；③ 各服务 `AGENTS.md` 是否引用 guide。

- 外部 `config/application.yml` 用 `${DATABASE_URL}` / `${DB_USER}` / `${DB_PASSWORD}` 占位符，但 Spring 实际读 `SPRING_DATASOURCE_*` 环境变量，两个 `${}` 是死代码。

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


### 4.2 Onboarding / 运维 [待推进]

来自 `docs/onboarding/` 系列 TODO：
- 内网仓库凭证申请流程（Docker Registry / Maven）—— 待运维补充
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
| `test` | `contracts_check` | 所有 pipeline | `node scripts/contracts-gen.js --check` | 校验 `contracts/**` 生成代码未漂移 |
| `test` | `e2e_api_test` | 所有 pipeline（无 `rules:`） | `cd tests/e2e && pnpm run test:api` | 跨网关测真服务 |
| `test` | `e2e_ui_test` | 所有 pipeline（无 `rules:`） | `cd tests/e2e && pnpm run test:ui`（Playwright） | 需前端容器 |
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


---

## 7. 文档与架构冲突（[风险]）


两个 Spring Boot 3 Java 服务并存，**功能重叠、定位不清**：

|---|---|---|
| 端口 | 容器 8080 / 宿主机 6008 | 容器 8088 |
| 技术栈 | Spring Boot 3 + **Java 25** + **JPA** + Flyway + springdoc-openapi | Spring Boot 3 + MyBatis-Plus + Flyway + springdoc-openapi + flexmark-java |


**AI 工作要求**：动其中任一服务前，**必须**先确认：
- 需求落在哪条线上（PMD 路线 / 禅道替代路线）

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
   - `iam-service` 走另一路径：`common-telemetry` 与 `common-telemetry-axum` 依赖未显式开 `otlp` feature；`Cargo.toml` 自身直接依赖 `opentelemetry = "0.24"` + `tracing-opentelemetry = "0.25"`，**但是否实际注册 OTLP exporter 尚未独立核验**。
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
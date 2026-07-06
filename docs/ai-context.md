# AI Context — coder-micro-services

> **AI 协作者在本仓库工作前必须先读此文件**。
> 它是项目形态、约定与已知坑点的单一真相源。
> 最后更新：2026-07-06（Phase 8.3-fu2 ntf/cfg/file 闭环，Phase 8 整体 DONE）。

---

## 1. 项目概览

`coder-micro-services` 是一个低内存微服务平台，围绕 Vue 3 管理端前端与 8 个 Rust 后端服务（api-gateway + 7 个业务服务）构建。用 Consul 做服务发现，Redis 做 JWT 黑名单 / 缓存，MySQL 存持久化数据，SQLite 供 pure-admin 菜单配置服务使用。多语言 SDK（Java / .NET / Node）发布到内网 Maven 与 NuGet 仓库。

- **仓库布局**：monorepo，含 `services/`（Rust）、`frontend/`（Vue 3）、`libs/`（Rust 共享库，含 `common-telemetry`）、`tests/e2e/`（Playwright + Vitest）、`contracts/`（错误码 / 权限码 / 事件的单一真相源）、`infrastructure/`（Consul/Redis/MySQL + 可观测性 overlay）。
- **当前阶段**：Phase 3 完成 + OTel 扩展 + coder-new-service skill。13 项路线图任务全部交付（见 `docs/superpowers/specs/2026-06-25-roadmap-completion-summary.md`）。
- **分支策略**：所有工作直接在 `main` 上进行，一个任务一个 commit，不开 feature 分支。

---

## 2. 服务地图

| Service | Port | DB | Tech | Role | OTel |
|---------|------|----|----|------|------|
| `api-gateway` | 3000 | — | Axum | JWT 验签、Redis 黑名单、路由到下游 + 限流 + tracing 中间件 | ✅ |
| `token-service` | 6001 | MySQL | Axum | 签发 / 刷新 / 吊销 JWT | ✅ |
| `iam-service` | 6002 | MySQL | Axum | 用户、角色、权限、部门、认证、租户注册、跨租户（platform_admin `cross_tenant` flag via Phase 8.3）。**事件生产者**（iam_outbox_events → 经 `iam-outbox-publisher` bin 发往 RabbitMQ） | ✅ |
| `iam-outbox-publisher` | — | MySQL | bin（与 iam-service 同镜像） | 独立进程：轮询 iam_outbox_events，发布到 RabbitMQ events exchange，标记已发布。用 `SELECT … FOR UPDATE SKIP LOCKED` 保证多副本安全。 | — |
| `notification-service` | 6003 | MySQL | Axum | 公告 CRUD + WebSocket 推送。所有 handler 接 `Principal + TenantScope`，service 全部 `scope: &TenantScope` + `QueryBuilder.and_tenant`/`where_id`，**点查询 TenantScope 隔离已贯通**（Phase 8.3-fu2）。`is_admin()` 仍按角色码 `admin` / `notification.admin` 检查（**非 platform_admin**, 见 §7 坑点 21） | ✅ |
| `prue_backend` | 6004 | SQLite/MySQL | Axum | pure-admin 菜单配置 + 动态路由 | — |
| `audit-service` | 6005 | MySQL | Axum | 操作日志 + 登录日志、留存、合规导出。**事件消费者**（audit.iam-events 队列 → audit_events 幂等 upsert）。Phase 4 已移除 POST 写日志端点 — 只走事件。 | — |
| `config-service` | 6006 | MySQL | Axum | 命名空间、配置项、watch 通知。**V002 migration 加 `cfg_namespaces.tenant_id` BIGINT UNIQUE(tenant_id,app,env,profile)**；handler `Principal + TenantScope` extractor（首加 auth）；service 全部 `scope: &TenantScope` + `QueryBuilder`，**点查询 + namespace 拥有校验 + drop SQLite 一并落地**（Phase 8.3-fu2）。SQLite 已 drop（dev 期不用） | — |
| `file-service` | 6007 | MySQL | Axum | 文件元数据、预签名上传、下载。**V002 migration `file_files.tenant_id` VARCHAR→BIGINT 类型对齐**；模型 `tenant_id: i64`（对齐平台 BIGINT UNSIGNED 体系）；service 全部 `scope: &TenantScope` + `QueryBuilder`，**点查询 TenantScope 隔离已贯通 + drop SQLite**（Phase 8.3-fu2） | — |
| `rabbitmq` | 5672/15672 | — | RabbitMQ 3.13 | 事件总线 broker；拓扑通过 `infrastructure/config/rabbitmq/definitions.json` 预声明（events topic exchange + events.dlx + audit.iam-events 队列 + events.dlq） | — |

每个 Rust 服务都有：
- `Cargo.toml`，含 `[[bin]]` `dump-openapi`（utoipa 生成器）
- `src/{lib,main,openapi,error,config}.rs` + `routes/` + `services/`（或 `service/`）
- 启用 OTel 的服务用 `common-telemetry-axum` 做 HTTP tracing 中间件
- `migrations/{mysql,sqlite}/V{NNN}__*.sql`（iam / token 仍用旧式扁平 `migrations/` 目录）
- `AGENTS.md` 声明本服务约定、端点域、env、豁免项
- `tests/*.rs` 集成测试
- `Dockerfile`

---

## 3. 架构

```
                           ┌──────────────────┐
    browser / SDK ────────►│   api-gateway    │──── Consul service discovery
                           │   :3000          │──── Redis blacklist
                           │                  │──── MySQL (read token / user)
                           └────────┬─────────┘
                 ┌──────────────────┼──────────────────────┐
                 ▼                  ▼                      ▼
        token-service         iam-service        notification-service
        :6001                 :6002              :6003
                 │                  │                      │
                 ▼                  ▼                      ▼
               MySQL              MySQL                 MySQL
                                               + WebSocket ws://…/notification/ws
                 ┌──────────────────┼──────────────────────┐
                 ▼                  ▼                      ▼
          prue_backend       audit-service        config-service / file-service
          :6004              :6005                :6006 / :6007
                 │                  │                      │
                 ▼                  ▼                      ▼
            SQLite/MySQL          MySQL                  MySQL
```

**各层职责**：
- **gateway**：JWT RS256 验签、黑名单检查、过期检查、下游路由。无业务逻辑。
- **token-service**：只做签发/刷新/吊销。产出 `access_token` + `refresh_token`。
- **iam-service**：身份 + RBAC + 部门树。通过 `libs/jwt-ctx` 提供 `Principal` extractor。
- **业务服务**：各自拥有自己的限界上下文。彼此不直接调用 — 一律走网关。

---

## 4. ADR 索引

| ADR | 标题 | 核心经验 |
|-----|-------|-----------|
| ADR-001 | api-gateway 多文件路由配置 | 按 service 拆 `routes/*.router.toml`，取代单一大文件。更好 diff、更好合并。 |
| ADR-002 | 服务规范基线 | 8 项规范 + 豁免 + 3 个 CI 门禁。新服务复制 `services/agents-template.md`。 |

---

## 5. Spec 索引（本季度）

| 主题 | 文件 |
|-------|------|
| 服务规范清扫 (P1.3) | `2026-06-25-service-standard-sweep-design.md` |
| prue_backend 并发 (P1.1+P1.2) | `2026-06-25-prue-backend-concurrency-design.md` |
| Swagger utoipa 覆盖 (P2.1) | `2026-06-25-swagger-coverage-design.md` |
| 剩余 4 服务 Swagger (P2.5b) | `2026-06-25-p25b-remaining-utoipa-design.md` |
| OpenAPI 自动 dump (P2.2) | `2026-06-25-openapi-auto-dump-design.md` |
| Coder skills (P2.3) | `2026-06-25-coder-skills-design.md` |
| L4 触发器 (P2.4) | `2026-06-25-l4-triggers-design.md` |
| 前端验证 (P2.5) | `2026-06-25-p25-verification.md` |
| E2E 稳定性 (P3.3) | `2026-06-25-e2e-stability-design.md` |
| 知识沉淀 (P3.5) | `2026-06-25-knowledge-consolidation-design.md` |
| Phase 4 事件总线（架构） | `2026-06-26-phase4-event-bus-design.md` |
| Phase 4.0 表名重构 | `2026-06-26-phase4.0-table-naming-design.md` |
| Phase 4 事件总线（实现） | `2026-06-26-phase4-event-bus.md` (plans/) — RabbitMQ topic exchange + transactional outbox；iam 生产、audit 消费；经 `scripts/e2e-event-flow.js` 实测联通 |

更早的 spec 放在同一 `docs/superpowers/specs/` 目录，描述的是已交付的服务。

---

## 6. Skill 目录

### 项目 skill（`.agents/skills/`）

| Skill | 用途 |
|-------|---------|
| `coder-add-route` | 脚手架新路由 handler，带 utoipa 注解 + openapi.rs 注册 |
| `coder-migrate-db` | 脚手架 MySQL/SQLite 双版本迁移文件，带 up/down |
| `coder-new-service` | 脚手架全新 Rust 微服务，满足 8 项服务规范 |

### 平台 skill（`.trae/skills/`）

| Skill | 用途 |
|-------|---------|
| `coder-base-images-sync` | 同步基础 Docker 镜像到内网 registry |
| `coder-dev-bootstrap` | 搭建本地开发环境 |
| `coder-docker-pull` | 拉取最新 Docker 镜像 |
| `coder-e2e-test` | 对基础设施跑 e2e 测试 |
| `coder-lib-upgrade` | 升级共享 Rust 库 |
| `coder-service-audit` | 按 AGENTS.md 规范审计服务 |
| `plan-eng-review` | 工程经理模式 plan 评审 |

### Skill 如何触发

见 `.trae/rules/project_rules.md` 的路径触发器。AI 根据用户在改哪些文件来加载 skill。

---

## 7. 已知坑点

这些都是踩过的真实问题，省你一下午：

1. **`utoipa::OpenApi::to_yaml()` 需要 `yaml` feature** — `Cargo.toml` 必须有 `utoipa = { version = "5", features = ["axum_extras", "chrono", "yaml"] }`。缺了它 `ApiDoc::openapi().to_yaml()` 编译不过。

2. **本地 `cargo` 编译不过**，因为缺 `cc` 链接器。用 `node scripts/run-tests-docker.js <svc>`，或在 `zhcoder-docker-registry.com:8000/builder/rust:1.96-slim-musl` 容器内构建。

3. **`api-gateway` 黑名单测试是历史遗留失败** — 它要求宿主机 Redis 在 `127.0.0.1:6379`。与任何在途改动无关。用 Docker 跑（用测试 Redis）即可通过。

4. **手写 `openapi.yaml` 已废止** — 全部 4 个服务现在都从 utoipa 生成。要改 spec，改代码（`#[utoipa::path]`、`ApiDoc::openapi()`），再跑 `scripts/dump-openapi-specs.sh`。不要直接编辑 yaml。

5. **迁移是 forward-only** — 不要编辑已发布的迁移。新增 `V{N+1}__*.sql`。CI spec 检查对 prue_backend 同时校验 MySQL 与 SQLite 两个版本。

6. **JWT 上下文只用 `Authorization` header** — 见 `.trae/rules/auth-rules.md`。任何地方都不要读 `X-User-*` headers。用 `libs/jwt-ctx::Principal` extractor。

7. **utoipa feature 一翻动 `Cargo.lock` 就变** — 这是预期的。把它和 `Cargo.toml` 的改动一起 commit。

8. **`check-spec.js` 报缺少 `summary` 字段** — 那是 warning 不是 error。即使有 warning 检查也 exit 0。除非在乎 UX 否则不用加（utoipa 默认不生成 summary）。

9. **`scripts/contracts-gen.js --check` 会重新生成 `libs/common-error/src/codes.rs`** — 不要直接编辑该文件。改 `contracts/error-codes.yaml`，再重跑。

10. **`notification/ws` 的 WS 鉴权** — 网关验 JWT 并注入用户身份。浏览器在 WS upgrade 时自动带 `Authorization`。不要把 token 放 URL 里。

11. **OTel traces 靠 env 开启** — `common-telemetry` 只有在 `OTEL_EXPORTER_OTLP_ENDPOINT` 已设置**且**启用了 `otlp` feature 时才导出 span。两者缺一即为静默 stdout-only。4 个服务（api-gateway / token-service / iam-service / notification-service）两者都接了。其余（prue_backend / audit / config / file）需同时补 feature flag + env var。见 `docs/guides/otel-tracing.md`。

12. **`TraceContextPropagator` 只在有 OTel subscriber 时生效** — 单元测试里没有 OTel subscriber 时，`set_parent(traceparent)` 不会把 trace_id 带进 span context。中间件回退到随机 `TraceId::from_bytes`，测试仍可校验 `X-Trace-Id` header 形态。端到端链路验证需要设了 `OTEL_EXPORTER_OTLP_ENDPOINT` 运行。

13. **网关限流返回 429 带 `Retry-After`** — 查 `infrastructure/config/api-gateway/rate-limits.toml` 看按路由的覆盖配置。默认 100 rps / 200 burst；auth 端点更严（`/v1/iam/auth/login` 5 rps / 10 burst）。要加新路由限流，往那个 TOML 追加并重启网关。

13. **服务侧限流经 `common-ratelimit` 开启** — Redis 滑动窗口，跨副本共享。opt-in 模式见 `services/notification-service/src/rest.rs`。Redis 出错时 fail-open。与网关限流（P3.2 内存）不同。见 `libs/common-ratelimit/` 与 `docs/USER_MANUAL.md#7-rate-limiting`。

14. **灰度通过 Consul 里的 `canary` tag** — Consul 中打了 `canary` tag 的实例经 Nginx `split_clients` 承接约 10% 流量。用 `?canary=1` query 参数或 `X-Canary: true` header 强制走 canary 测试。见 `docs/guides/canary-rollout.md`。

15. **双层限流** — 网关（内存 token bucket，P3.2）+ 服务侧（Redis 滑动窗口，P3.2b）叠加。网关做粗粒度抗 DDoS，服务做细粒度业务限流（如 notification-service 的 `每管理员每分钟 5 条公告`）。服务侧 `common-ratelimit` 库供任何服务 opt-in；模式见 `services/notification-service/src/rest.rs`。Redis 出错时 fail-open（记 warn，放行请求）。

16. **Smoke 子集用 `E2E_SMOKE=1`** — 只跑带 `smoke: true` 标记的用例。8 个 API + 2 个 UI 用例约 2 分钟，全量 API 约 5 分钟。CI 永远跑全量；smoke 用于开发迭代。见 `docs/guides/testing.md`。

17. **E2E 预检健康检查** — `tests/e2e/shared/health.ts` 在测试跑之前先探 `/health` 和 `/iam/health`。若基础设施没起，你会拿到明确报错指向 `infrastructure/docker-compose.yml`，而不是连锁失败。
18. **共享 MySQL `superpowers` 库 — 所有表必须带服务前缀**（Phase 4.0）。`iam_` / `audit_` / `token_` / `ntf_` / `cfg_` / `file_` / `prue_`。CI 门禁 `node scripts/check-db-naming.js` 强制。新建迁移时给表名加前缀并更新所有 `sqlx::query*` 调用。完整规则见 `contracts/db-conventions.md`。

19. **事件总线 = RabbitMQ + transactional outbox**（Phase 4）。跨服务「通知审计日志」类的写操作走事件总线，不走直连 HTTP。发事件：在单个 sqlx 事务里，写业务行**同时**调用 `common_event::MySqlOutboxRepo::enqueue(&mut tx, &Envelope::new("<svc>", payload))`。outbox-publisher bin（独立进程、同镜像）轮询该表并发布到 `events` topic exchange。消费者按 routing-key 模式绑定队列（如 `iam.#`）。所有消费者必须按 `event_id` **幂等**（我们提供 at-least-once）。`common-event` 里的 Value-payload Event impl **只能消费** — 永远不要 publish `Envelope<serde_json::Value>`（会被路由到 `"*"`）。端到端冒烟：`node scripts/e2e-event-flow.js`。

---

## 8. 常用命令

```bash
# CI 门禁（push 前跑）
node scripts/check-service-standard.js
node scripts/check-spec.js
node scripts/contracts-gen.js --check
node scripts/check-db-naming.js

# Cargo 测试（docker）
node scripts/run-tests-docker.js audit-service config-service file-service prue_backend iam-service notification-service token-service

# 重新生成 OpenAPI yaml（仅当 handler 注解变了）
scripts/dump-openapi-specs.sh

# E2E
cd tests/e2e && pnpm install
pnpm -C tests/e2e test:smoke        # ~2 分钟 smoke 子集
pnpm -C tests/e2e test:smoke:ui    # UI smoke 子集
pnpm -C tests/e2e test:api          # 全量 API
pnpm -C tests/e2e test:ui           # 全量 UI（需前端在跑）

# 构建镜像
node scripts/base-images-sync.js --check

# 可观测性 overlay（Prometheus + Grafana + OTel Collector + Jaeger）
docker compose -f infrastructure/docker-compose.yml up -d
scripts/start-observability.sh
# 然后访问：
#   Prometheus :9090  (metrics)
#   Grafana    :3001  (admin/admin) — dashboards 自动加载
#   Jaeger     :16686 (traces UI)
#   OTel collector OTLP gRPC :4317

# 灰度发布（10% canary tag）
# 见 docs/guides/canary-rollout.md
docker exec consul consul tags -add -service audit-service -tag canary -id <instance-id>
# 或在 compose 里给 canary 实例打 `canary` label → 流量切分

# 安装 pre-commit hook（一次性）
pre-commit install
# 或回退方案：
cp scripts/git-hooks/pre-commit .git/hooks/pre-commit && chmod +x .git/hooks/pre-commit
```

---

## 9. CI 门禁

| 门禁 | 脚本 | 触发 | 状态 |
|------|--------|---------|--------|
| Service Standard | `node scripts/check-service-standard.js` | `services/**` | ✅ 全部 8 服务 100% |
| Spec Check | `node scripts/check-spec.js` | `services/*/openapi.yaml` | ✅ exit 0 |
| Contracts Check | `node scripts/contracts-gen.js --check` | `contracts/**` | ✅ exit 0 |
| **DB Naming** | `node scripts/check-db-naming.js` | `services/*/migrations/**`、`services/*/src/**/*.rs` (sqlx) | ✅ 全部 7 服务已加前缀 (Phase 4.0) |
| OpenAPI Drift | `scripts/dump-openapi-specs.sh` + `git diff --exit-code` | `services/*/openapi.yaml` | ✅ 全部 4 服务同步 |
| Rust Tests | `node scripts/run-tests-docker.js <svc>` | `services/<svc>/**` | ✅（api-gateway 黑名单测试历史遗留失败） |
| E2E API | `pnpm -C tests/e2e test:api` | `tests/e2e/api/cases/*.yaml` | ✅ |
| E2E UI | `pnpm -C tests/e2e test:ui` | `tests/e2e/ui/cases/*.yaml` | ✅（需前端） |
| E2E Smoke | `pnpm -C tests/e2e test:smoke` | 预部署 / 开发迭代 | ✅ 8 API + 2 UI 用例，约 2 分钟 |

所有门禁在 GitLab CI `test` 阶段运行；失败阻断 build/deploy。

---

## 10. 导航地图

- **要加路由？** → `.agents/skills/coder-add-route/SKILL.md`
- **要加迁移？** → `.agents/skills/coder-migrate-db/SKILL.md`
- **要新建服务？** → `.agents/skills/coder-new-service/SKILL.md`
- **要改契约？** → 编辑 `contracts/*.yaml`，跑 `scripts/contracts-gen.js`
- **要评审 plan？** → `.trae/skills/plan-eng-review/SKILL.md`
- **要审计服务？** → `.trae/skills/coder-service-audit/SKILL.md`
- **要理解一个旧决策？** → `docs/decisions/ADR-*.md`
- **要某服务的约定？** → `services/<svc>/AGENTS.md`
- **要调试请求链路？** → `docs/guides/otel-tracing.md` (Jaeger)
- **要做灰度发布？** → `docs/guides/canary-rollout.md`
- **要搭可观测性？** → `scripts/start-observability.sh` + `docs/USER_MANUAL.md#observability`
- **要写/跑测试？** → `docs/guides/testing.md`
- **要全局概览？** → `docs/USER_MANUAL.md` 或 `docs/ai-context.md`
- **要知道接下来做什么？** → `docs/superpowers/plans/2026-06-25-overall-roadmap.md`

---

_最后更新：2026-07-06（Phase 8.3-fu2 ntf/cfg/file 点查询 TenantScope 隔离完成，Phase 8 整体闭环）。新增 service / skill / ADR 时，请同步更新此处对应章节。_

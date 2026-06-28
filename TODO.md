# TODO

（已清空：原 P1.1 prue_backend 草稿并发控制 与 P1.2 sanitize active routes 输出
已于 2026-06-25 完成，详见
`docs/superpowers/specs/2026-06-25-prue-backend-concurrency-design.md`
与对应实现 commit。）

---

## 架构师评审遗留事项（2026-06-26 全量功能点扫描产出）

> 来源：对项目做全量功能点扫描后，以「企业级生产微服务平台」标准做的架构师 review。
> 每一项都需要先 brainstorm → 写 spec → 写 plan → 实现，**不要直接动代码**。
> 路线图按 Phase 4-8 推进，每个 Phase 单独一个 spec。

### Phase 4 — 异步事件总线（P0，解锁最终一致性 / 削峰 / 解耦）

> 中间件 = RabbitMQ；Topology = 单 topic exchange + routing-key；Outbox = 每服务一张表 + 同事务；Publisher = 独立二进制 + 多副本 `SELECT ... FOR UPDATE SKIP LOCKED`；首批事件 = `iam.user.created` / `iam.user.deleted` / `iam.login.succeeded`；首批闭环 = iam → audit（旧 HTTP 直接删）。
> Spec: `docs/superpowers/specs/2026-06-26-phase4-event-bus-design.md`

#### Phase 4.0 — 数据表命名规范统一（**已完成并合并**）

> 现状：所有 Rust 服务共用 MySQL `superpowers` 库，且无任何表名隔离约定，已经出现 iam/audit 两个 `login_logs` 撞名 —— 谁先跑迁移谁占位。继续往里加 outbox 等表会让越挖越大。
> Spec: `docs/superpowers/specs/2026-06-26-phase4.0-table-naming-design.md`
> 计划 + 进度: `docs/superpowers/plans/2026-06-26-phase4.0-table-naming.md`, `.superpowers/sdd/progress.md`

- [x] 新建 `contracts/db-conventions.md`，定前缀映射：`iam_/audit_/token_/ntf_/cfg_/file_/prue_`（commit `f3e9aa7`）
- [x] 排查 dev 环境 `login_logs` 撞名实际情况，确定归属（结论：双方各自一份，详见 `.superpowers/sdd/task-0-baseline.md`）
- [x] 各服务 `V00X__rename_tables.sql` 一次性 RENAME 存量表（采用 empty-db-regenerate 策略：把 V001 内表名前缀化）
- [x] sqlx 查询、e2e fixture、测试 setup、文档同步改名
- [x] `scripts/check-db-naming.js` CI gate（commit `143101c`）
- [x] `agents.md` / `services/<svc>/AGENTS.md` 更新规范（commit `2049a33`）
- [x] `coder-migrate-db` skill 模板加前缀默认值

#### Phase 4 主任务（**已完成并合并**）

> 17 任务全部完成 + 全面 review 修两个 Critical 落地。详见 `.superpowers/sdd/progress.md` 与 `docs/superpowers/plans/2026-06-26-phase4-event-bus.md`。

- [x] 基础设施：`infrastructure/docker-compose-infra.yml` 加 RabbitMQ + `definitions.json` 预声明 topology（commits `c52f401`, `b0eb5e2`）
- [x] `contracts/events.yaml` 启用首批三事件 + envelope schema（commit `2fe432f`）
- [x] `scripts/contracts-gen.js` 扩展：events.yaml → Rust event struct + routing-key 常量（commit `8e9823b`）
- [x] **`libs/common-event` 完整抽象**：Publisher / Consumer / Envelope / OutboxRepo<DB> trait + lapin RabbitMQ 实现（commits `265adcc` … `e95069f` + minor `8a014de`）
- [x] iam-service 加 `iam_outbox_events` 表 + 业务事务里写 outbox（commits `9f8e06b`, `dc048fa`）
- [x] iam-service 加 `[[bin]] outbox-publisher`（同镜像切 CMD）（commit `7808a7a`）
- [x] audit-service 加 `audit_events` 表 + RabbitMQ consumer + 删除旧 HTTP 写入端点（commits `0749893`, `ed2f63a` + minors `1577ed9`, `7852718`）
- [ ] ~~audit-service 补 OTel 接入（验收要求端到端 trace 链）~~  — **推迟到 Phase 6**；envelope.trace_id 字段已在 producer 侧预留，consumer 侧 OTel wiring 待 Phase 6 统一处理
- [x] e2e 用例 `scripts/e2e-event-flow.js`（commit `945ff46`）— runner 不支持直接 DB 断言，改 standalone live smoke
- [x] 验收：RabbitMQ 拔掉/恢复、consumer panic → DLQ、Jaeger 端到端 trace 链
  - ✅ Broker-bounce 恢复实测：停 RabbitMQ → 创建 user → 重启 → publisher 10s 内追上（commit `fab9cd2` 修复 C1）
  - ✅ SIGTERM/SIGINT 双处理：docker stop 走 SIGTERM，现在能正确触发 "finish current batch then exit"（commit `fab9cd2` 修复 C2）
  - ⚠️ DLQ replay / shovel：未实测；Topology 已就绪（`x-dead-letter-exchange: events.dlx`），手动 shovel 验证留给 Phase 5
  - ⚠️ Jaeger 端到端：依赖 audit-service OTel（Phase 6）

#### Phase 4 后续（**Phase 4.x 小迭代，每个子任务一个 spec —— 4.1~4.4 已全部完成并合并**）

> 手工端到端验收场景见 `test-case/`（00-setup + 41/42/43/44，逐场景步骤表）。
> 仅剩 4.3-fu*/4.4-fu* 为非阻塞 follow-up。

- [x] **4.1** notification-service 订阅 `iam.user.created` → 站内信欢迎（commits `94593df`..`38a5267`；plan: `2026-06-27-phase4.1-notification-welcome-template-plan.md`）。手工验收：`test-case/41-notification-welcome.md`。
- [x] **4.2** 落地 `iam.login.failed` + 简易风控消费者（commits `492abd8`..`01f7723`；spec: `2026-06-27-phase4.2-login-failed-risk-design.md`，plan: `2026-06-27-phase4.2-login-failed-risk-plan.md`）。手工验收：`test-case/42-login-failed-risk.md`。
- [x] **4.3** 落地 `notification.announcement.published`，WebSocket 广播改由事件驱动（commits `c5f423e`..`d9961d0`；spec: `2026-06-27-phase4.3-announcement-published-ws-design.md`，plan: `2026-06-27-phase4.3-announcement-published-ws-plan.md`；本期仅 user-scope）。手工验收：`test-case/43-announcement-ws.md`。
  - [ ] **4.3-fu1** 大规模扇出时 `publish_announcement` 的 delivery 写入改批量多值 INSERT（当前逐条 INSERT IGNORE，复用 `db.rs:insert_deliveries` 模式；user-scope 规模小不成问题，role/dept 扇出落地后才需要）。依赖：4.3-fu2。
  - [ ] **4.3-fu2** role/dept 接收人解析落地：为 `recipient_resolver::expand_local` 接上数据源（iam「按 role/dept 列用户」端点或成员映射），让 `publish_announcement` 解析 role/dept scope（当前仅 warn 跳过，按角色/部门发的公告实际无人收到）。依赖：iam 成员查询能力；可与 4.4 成员视图协同。
  - [ ] **4.3-fu3** 4.1 welcome 路径（`announcement_service.rs:ensure_welcome_for_user`）多副本化：改走事件驱动 WS 推送而非进程内 `hub.publish`（消除最后一处进程内直推；delivery 已落库故 reload 可见，影响小）。依赖：4.3 的 ephemeral consumer 基建。
- [x] **4.4** `iam.permission.changed` 事件 + perm-epoch 令牌失效（双写）（commits `512b389`..`4230b06`，最终对齐 prefix 修复见 4.4 分支末尾 commit；spec: `2026-06-27-phase4.4-permission-changed-design.md`，plan: `2026-06-27-phase4.4-permission-changed-plan.md`）。iam 在 assign_permissions / role 禁删 / permission 禁删 同事务展开 user_ids + 发事件 + 直写 Redis `perm_epoch:{uid}`；gateway 比对 `iat < perm_epoch` → 401，并消费事件用 set-if-greater 兜底补写。无服务端权限缓存可失效，故走令牌失效（架构评审已否决「token 落库反查拉黑」方案）。手工验收：`test-case/44-permission-epoch.md`。
  - [ ] **4.4-fu1** 新增 `PUT /iam/users/:id/roles` 写端点并接入失效（当前 user→role 仅在建用户时设，已登录用户改不了角色）。
  - [ ] **4.4-fu2** actor_id 透传：把 Principal 从 routes 传到 role/permission service，使 `iam.permission.changed.actor_id` 非空（当前恒 None）。
  - [ ] **4.4-fu3** gateway perm_epoch 本地内存短缓存，降低每请求 Redis 读频。
  - [ ] **4.4-fu4** `PermissionService::create` 仍用 MySQL 不支持的 `INSERT ... RETURNING`（与 Task 8 修掉的 update 同模式 bug，create 不在 4.4 触发路径故未改）——统一改为 INSERT + re-SELECT。

#### Phase 4 broad review 遗留（cheap fixes 落地，剩下记录在 `.superpowers/sdd/progress.md`）

- [x] **C1** publisher 无 RabbitMQ reconnect（修复于 `fab9cd2`，已 live verified）
- [x] **C2** ctrl_c 不捕 SIGTERM（修复于 `fab9cd2`，现双捕）
- [x] **I3** `OutboxPublisher::run()` bug-shape（doc 标记为 TEST HELPER ONLY）
- [ ] **I1** consumer 瞬时错误直接 DLQ（应区分 transient/permanent，retry with TTL） — Phase 5
- [ ] **I2** audit-service 仍有 `create_operation_log`/`create_login_log` 死方法 + `CreateOperationLog`/`CreateLoginLog` 孤儿 schema — 后续清理
- [ ] **I4** envelope.trace_id producer 侧未填充（docs 说已填，实际没填） — Phase 6 一起
- [ ] **I5** `MySqlOutboxRepo::new(pool.clone())` per-call — 移到 `AppState` 优化
- [ ] **I6** `iam_outbox_events` 无归档 cron（spec §7.3 要求 24h archive） — Phase 5

### Phase 5 — 服务间安全 & 韧性（P1）

> **已拆分为两个独立子项目（2026-06-28 brainstorm + plan-review）：**
> - **A = mTLS / 服务网格**：待 brainstorm，无 spec。重型，涉及基础设施选型（证书分发、部署形态）。
> - **B = 韧性三件套**：spec 已就绪 + review 结论已 bake，见
>   `docs/superpowers/specs/2026-06-28-phase5-resilience-triad-design.md`，待 writing-plans。

- [ ] **A — mTLS / 服务网格**（独立子项目，待 brainstorm）
  - [ ] 评估 Consul Connect vs Linkerd2（侧重内存占用与运维复杂度）
  - [ ] 服务间通信切换为 mTLS（rustls 已就位）
  - [ ] 淘汰应用层 `require_service_token` 方案

- [ ] **B — 韧性三件套**（spec 已就绪，**核心已完成并合并**，仅剩 B-fu1）
  - [x] 在 `libs/common-http` 提供通用 `ResilientClient`（包 reqwest）：超时 + 重试 + 熔断
  - [x] 本期接 gateway `forward_to_service`：每下游 service 独立 breaker，状态经 `ResilienceObserver` → Prometheus
  - [x] 删除并重写未使用的韧性脚手架（axum/benchmarks/rate_limit/middleware/service_discovery/circuit_breaker/retry/config）
  - [ ] **B-fu1** 韧性三件套扩面到其余服务间直调（token→user、各服务 `common-http::HttpClient` / `common::http::client()` 调用点）。
        当前仅 gateway outbound 受保护；服务间直调仍裸 reqwest。依赖：B 实现完成。起点：grep `common-http::HttpClient` 与 `common/src/http.rs::client()` 调用方。


### Phase 6 — 配置 & 可观测性补齐（P1）

- [ ] **config-service 自举（dogfooding）**
  - [ ] `libs/common-config` 新增 ConfigService Provider，启动拉取 + 长轮询/SSE watch
  - [ ] 所有服务从读本地 yaml 切到读 config-service（保留本地兜底）
  - [ ] 敏感配置分离到 Vault / Consul KV + age 加密

- [ ] **OTel 全覆盖**
  - [ ] 剩余 4 个服务接入 OpenTelemetry：`audit-service` / `config-service` / `file-service` / `prue_backend`
  - [ ] 端到端 trace 链验证（gateway → iam → audit 事件链）

### Phase 7 — 生产级部署（P2）

- [ ] **K3s / Kubernetes 化**
  - [ ] 服务 chart / manifest 编写（替代 `docker-compose` 生产用途）
  - [ ] 三级探针：startup / liveness / readiness
  - [ ] ConfigMap + Secret + RBAC 集成
  - [ ] HPA 基于 CPU + 自定义 metrics（QPS / queue depth）

- [ ] **灰度发布升级**
  - [ ] 引入 Argo Rollouts 或 Flagger，替换 Consul tag + Nginx split_clients
  - [ ] 指标驱动的金丝雀（错误率 / P99 自动回滚）

### Phase 8 — 多租户基础（P2，按产品形态择机）

- [ ] **tenant_id 全链路贯通**
  - [ ] `libs/jwt-ctx::Principal` 增加 `tenant_id` 字段
  - [ ] 业务表 schema 增加 `tenant_id` + 复合索引（迁移分批进行）
  - [ ] `libs/common-db` 提供 `TenantScopedQuery` helper，extractor 自动注入 WHERE

### Phase 9 — 小问题清单（P3，零散收尾）

- [ ] `prue_backend` 评估是否去掉 SQLite 分支，统一 MySQL（SQLite 仅留 dev 模式）
- [ ] `events.yaml` — **Phase 4 已落地**（`docs/superpowers/specs/2026-06-26-phase4-event-bus-design.md` + `contracts/codegen/gen-rust-events.js`），首批三事件 `iam.user.created`/`iam.user.deleted`/`iam.login.succeeded` 已 codegen + producer/consumer 闭环验证 live
- [ ] 健康检查补齐三级探针（目前仅 liveness）
- [ ] 数据库 schema 当前版本/历史查询端点（`/internal/migrations/state`）
- [ ] 密钥轮换机制 SOP（JWT 私钥、DB 密码、API key）
- [ ] gateway 限流统一走 Redis，淘汰 in-memory 实现避免双实现困惑
- [ ] 评估自研 router 替换为 Envoy / Pingora 的成本/收益

---

## 新员工手册待补充（docs/onboarding/）

以下 TODO 项来自 `docs/onboarding/` 系列文档，需要人工补充：

### 基础设施与权限

- [ ] **内网仓库凭证申请**：Docker Registry (`zhcoder-docker-registry.com:8000`)、Maven (`http://mvn.kudi.repos`)、NuGet (`http://nuget.kudi.repos`) 的账号申请流程 — **待运维补充**
- [ ] **生产数据迁移 SOP**：生产环境数据库迁移的备份、回滚、灰度验证标准流程 — **待运维补充**
- [ ] **集中式日志收集**：生产环境日志收集方案（ELK / Loki / 阿里云 SLS）、查询入口、权限申请 — **待运维补充**

### 团队规范

- [ ] **求救渠道**：技术问题 IM 群号/频道名、on-call 值班表、紧急故障联系人、升级路径 — **待团队负责人补充**
- [ ] **commit message 规范**：是否使用 Conventional Commits、PR 标题格式、是否需关联 issue/ticket — **待规范人补充**

### 技术细节

- [ ] **config-service 使用方式**：如何通过 config-service API 读写配置、各服务如何集成、配置优先级 — **待熟悉 config-service 的开发者补充**
- [ ] **前端登录态刷新机制细节**：Token 刷新触发条件、并发刷新处理、自动登出策略的完整实现 — **待前端负责人确认（目前已基于代码写出基本逻辑，需确认边界情况）**

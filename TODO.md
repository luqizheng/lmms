# TODO

> 已完成的项归档于 [done.md](./done.md)，勿手动编辑。

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
- [x] **`libs/digua_common-event` 完整抽象**：Publisher / Consumer / Envelope / OutboxRepo<DB> trait + lapin RabbitMQ 实现（commits `265adcc` … `e95069f` + minor `8a014de`）
- [x] iam-service 加 `iam_outbox_events` 表 + 业务事务里写 outbox（commits `9f8e06b`, `dc048fa`）
- [x] iam-service 加 `[[bin]] outbox-publisher`（同镜像切 CMD）（commit `7808a7a`）
- [x] audit-service 加 `audit_events` 表 + RabbitMQ consumer + 删除旧 HTTP 写入端点（commits `0749893`, `ed2f63a` + minors `1577ed9`, `7852718`）
- [x] ~~audit-service 补 OTel 接入（验收要求端到端 trace 链）~~ — **由 Phase 6-A T2 覆盖**（commits `6f7cae1`, `5cdc1d8`）：audit consumer 用 envelope.trace_id 设 span parent（`consumer_span.rs`）+ `init_tracing` + middleware 接入。
- [x] e2e 用例 `scripts/e2e-event-flow.js`（commit `945ff46`）— runner 不支持直接 DB 断言，改 standalone live smoke
- [x] 验收：RabbitMQ 拔掉/恢复、consumer panic → DLQ、Jaeger 端到端 trace 链
  - ✅ Broker-bounce 恢复实测：停 RabbitMQ → 创建 user → 重启 → publisher 10s 内追上（commit `fab9cd2` 修复 C1）
  - ✅ SIGTERM/SIGINT 双处理：docker stop 走 SIGTERM，现在能正确触发 "finish current batch then exit"（commit `fab9cd2` 修复 C2）
  - ⚠️ DLQ replay / shovel：未实测；Topology 已就绪（`x-dead-letter-exchange: events.dlx`），手动 shovel 验证留给 Phase 5
  - ⚠️ Jaeger 端到端：audit consumer OTel 已接（Phase 6-A T2），但 live Jaeger 端到端 trace 链未实测（独立 follow-up）

#### Phase 4 后续（**Phase 4.x 小迭代，每个子任务一个 spec —— 4.1~4.4 已全部完成并合并**）

> 手工端到端验收场景见 `test-case/`（00-setup + 41/42/43/44，逐场景步骤表）。
> 仅剩 4.3-fu*/4.4-fu* 为非阻塞 follow-up。

- [x] **4.1** notification-service 订阅 `iam.user.created` → 站内信欢迎（commits `94593df`..`38a5267`；plan: `2026-06-27-phase4.1-notification-welcome-template-plan.md`）。手工验收：`test-case/41-notification-welcome.md`。
- [x] **4.2** 落地 `iam.login.failed` + 简易风控消费者（commits `492abd8`..`01f7723`；spec: `2026-06-27-phase4.2-login-failed-risk-design.md`，plan: `2026-06-27-phase4.2-login-failed-risk-plan.md`）。手工验收：`test-case/42-login-failed-risk.md`。
- [x] **4.3** 落地 `notification.announcement.published`，WebSocket 广播改由事件驱动（commits `c5f423e`..`d9961d0`；spec: `2026-06-27-phase4.3-announcement-published-ws-design.md`，plan: `2026-06-27-phase4.3-announcement-published-ws-plan.md`；本期仅 user-scope）。手工验收：`test-case/43-announcement-ws.md`。
  - [x] **4.3-fu1** 大规模扇出时 `publish_announcement` 的 delivery 写入改批量多值 INSERT（QueryBuilder::push_values + insert_deliveries_batch helper；2 处调用点改走 helper；announcement_recipient_test 加 100-user 回归）
  - [x] **4.3-fu2** role/dept 接收人解析落地（spec: docs/superpowers/specs/2026-07-01-phase4.4-fu2-actor-id-phase4.3-fu2-recipient-resolver-design.md, plan: docs/superpowers/plans/2026-07-01-phase4.4-fu2-actor-id-phase4.3-fu2-recipient-resolver.md）。Post-merge 修复：commit `05b3c22` 将 `IamMemberClient::by_role/by_dept` 的 URL 追加 `?size=1000`（消除 >100 人角色/部门名单静默截断）。
  - [x] **4.3-fu3** 4.1 welcome 路径多副本化（commit `547e8c0`）：`ensure_welcome_for_user` 删进程内 `hub.publish`，同事务 outbox enqueue `NotificationAnnouncementPublished{user_ids:[user_id]}`；WS 推送由 4.3 既有 ephemeral 消费者扇出到每副本，仅连着用户 WS 的副本真正送达。Spec: `docs/superpowers/specs/2026-07-08-phase4.3-fu3-welcome-fanout-design.md`。验证：`scripts/run-tests-docker.js notification-service` 48/48 passed；service-standard / db-naming / contracts-gen --check 全部 clean。
  - [x] **4.3-fu4** consumer `run()` block-on-disconnect 修复：审计/通知两个 durable-queue consumer 的 30s 重订阅循环换为持 `RabbitMqConsumer` + `while consumer.connection().status().connected() { sleep(5s) }`（复用 4.3 `announcement_run` 模式）；durable 队列在 broker 侧保留，未 ack 消息断线重订阅后 broker 重投。每 30s queue_declare/bind/basic_consume churn 消除。`services/audit-service/src/consumer.rs::run()` + `services/notification-service/src/consumer/mod.rs::run()`（spec `2026-06-28-phase4-cheap-fixes-design.md` §2.3）。
- [x] **4.4** `iam.permission.changed` 事件 + perm-epoch 令牌失效（双写）（commits `512b389`..`4230b06`，最终对齐 prefix 修复见 4.4 分支末尾 commit；spec: `2026-06-27-phase4.4-permission-changed-design.md`，plan: `2026-06-27-phase4.4-permission-changed-plan.md`）。iam 在 assign_permissions / role 禁删 / permission 禁删 同事务展开 user_ids + 发事件 + 直写 Redis `perm_epoch:{uid}`；gateway 比对 `iat < perm_epoch` → 401，并消费事件用 set-if-greater 兜底补写。无服务端权限缓存可失效，故走令牌失效（架构评审已否决「token 落库反查拉黑」方案）。手工验收：`test-case/44-permission-epoch.md`。
- [x] **4.4-fu1** 新增 `PUT /iam/users/:id/roles` 写端点并接入失效（当前 user→role 仅在建用户时设，已登录用户改不了角色）。spec: docs/superpowers/specs/2026-07-01-phase4.4-fu1-user-roles-assign-design.md, plan: docs/superpowers/plans/2026-07-01-phase4.4-fu1-user-roles-assign.md
- [x] **4.4-fu2** actor_id 透传（spec: docs/superpowers/specs/2026-07-01-phase4.4-fu2-actor-id-phase4.3-fu2-recipient-resolver-design.md, plan: docs/superpowers/plans/2026-07-01-phase4.4-fu2-actor-id-phase4.3-fu2-recipient-resolver.md）
- [x] **4.4-fu3** gateway perm_epoch 本地内存短缓存，降低每请求 Redis 读频。（commit `0f14e1a`）
  - [x] **4.4-fu4** `PermissionService::create` 仍用 MySQL 不支持的 `INSERT ... RETURNING`（与 Task 8 修掉的 update 同模式 bug，create 不在 4.4 触发路径故未改）——统一改为 INSERT + re-SELECT。

#### Phase 4 broad review 遗留（cheap fixes 落地，剩下记录在 `.superpowers/sdd/progress.md`）

- [x] **C1** publisher 无 RabbitMQ reconnect（修复于 `fab9cd2`，已 live verified）
- [x] **C2** ctrl_c 不捕 SIGTERM（修复于 `fab9cd2`，现双捕）
- [x] **I3** `OutboxPublisher::run()` bug-shape（doc 标记为 TEST HELPER ONLY）
- [x] **I1** consumer 瞬时错误直接 DLQ（修复于 Phase 6-A T3：HandlerError 扩 Transient/Permanent 分支 + republish-and-ack 阈值 3；commit `10e4b07`）
- [x] **I2** audit-service 仍有 `create_operation_log`/`create_login_log` 死方法 + `CreateOperationLog`/`CreateLoginLog` 孤儿 schema — 后续清理
  - [x] **I2-fu1** ~~`scripts/openapi-codegen.js:90` 硬编码~~ 已修复（commit `0412751`）：脚本改为 spec-driven —— 读 `spec.components.schemas` 生成 types（按 `required`/nullable）、读 `spec.paths` 生成 client 方法。下次 `pnpm sdk:generate:audit` 不会再回写 commit `89906cf` 删掉的 `CreateOperationLog`/`CreateLoginLog`；`LoginLog.user_id?: string;` 现在与 Rust `Option<String>` 一致。已用 audit/config/file 三个 spec 验证。
- [x] **I4** envelope.trace_id producer 侧未填充（修复于 Phase 6-A T1：`libs/digua_common-telemetry/src/outbox.rs` 在 enqueue 内自动注入；commits `798683b`, `4a4ff51`）
- [x] **I5** `MySqlOutboxRepo::new(pool.clone())` per-call — 移到 `AppState` 优化
  - [x] **I5-fu1** 已修复（commit `5bfb2da` + 行号修正 `e78aad8`）：iam 剩余 7 处 `MySqlOutboxRepo::new(pool.clone())` 全部改走 `state.outbox_repo.as_ref()`。
    - `auth_svc.rs:115/356`（login_success / login_failed）：直接换 `digua_common_event::OutboxRepo::enqueue(self.state.outbox_repo.as_ref(), ...)`
    - `perm_change.rs:39` helper 签名 `db: &MySqlPool` → `repo: &MySqlOutboxRepo`；5 个 caller（`role_svc.rs:148/185/220` + `permission_svc.rs:208/253`）改传 `self.state.outbox_repo.as_ref()`
    - 回归：`permission_changed_test` 7/7 PASS（testcontainers），workspace build clean
- [x] **I6** `iam_outbox_events` 无归档 cron（spec §7.3 要求 24h archive） — 修复于 Phase 6-A T4：archive_loop + purge_loop + `iam_outbox_events_archive` 表（commit `5389b24`）

### Phase 5-A-2 — 放弃 Consul Connect, 改用 registrator（2026-07-07 完成）

> 本 session 落地。撤 Consul Connect（sidecar / intentions / connect proxy / consul-dataplane），
> 用 `gliderlabs/registrator` 项目级注册 + 服务间直连 HTTP + `Authorization: Bearer <jwt>` 透传。
> Spec: `docs/superpowers/specs/2026-07-07-deprecate-consul-connect-use-registrator-design.md`
> Plan: `docs/superpowers/plans/2026-07-07-deprecate-consul-connect.md`
> 10 个 commit，全部在 main 上。

- [x] **5A-2-fu1** `registrator:master` 镜像固化到 sha256 digest（当前用 `:master` tag，非不可变构建）。锁版本防破坏性变更。（不改：master 标签可接受）
- [~~] **5A-2-fu2** ~~`docker-example/` 目录为 root-owned，无 compose 文件；要么 chown 后补 README 说明废弃，要么清空。当前为死目录。~~（已删除）

### Phase 5-B — JWT 签名验证 + service-jwt 机制（2026-07-07 主体完成，2026-07-08 5B-fu3/fu5/fu6 追加）

> 14 个 commit（主体）+ 13 个 commit（5B-fu3 事件化）+ 1 个 commit（5B-fu5 CI）+ 2 个 commit（5B-fu6 回归测试 + TODO 归档）。
> 本 section 保留 fu1~fu6 状态快照；其余已移交 done.md。
> Spec: `docs/superpowers/specs/2026-07-07-phase5b-jwt-signature-and-service-jwt-design.md`
> Plan: `docs/superpowers/plans/2026-07-07-phase5b-jwt-signature-and-service-jwt.md`

- [x] **5B-fu1** `Principal::from_jwt_payload`（无验签路径）加 `#[deprecated]`。当前保留为测试用，生产应走 `from_token`。小改。
- [x] **5B-fu2** `api-gateway` 统一到 `digua_common-jwt::JWKSCache`（当前用自己的 `JwtValidator` / `jwt_simple`）。架构债。（commits `0f14e1a` … `d295987`）
- [x] **5B-fu3** `notification → iam` 事件化 — 完整落地：spec + plan + T1-T10 全部 commit（`a284e4e`…`d070914`）。notification→iam 同步 HTTP 已消除（publish_announcement 走 `resolve_users_by_scopes` 本地 SQL）；iam 7 emit 点同事务 outbox；V003 ntf_user_projection 单表 projection + bootstrap 端点 + durable consumer `notification.membership-events`（3 typed subscriber）；`IamMemberClient::{by_role,by_dept,expand_scopes}` 标 `#[deprecated]` + warn log。integration tests: notification 45/45 pass; iam 38/39 (1 pre-existing SIGABRT dump-openapi)。
- [x] ~~**5B-fu4** TLS termination~~ — **延后至 Roadmap 2.0**（1.0 内不增加安全相关功能）
- [x] **5B-fu5** CI 对齐 5B spec §7.2：`cargo test --workspace --lib`（commit `4e1c73c`；本地实测 74 passed / 0 failed，需 `CARGO_TARGET_DIR` 指向非 tmpfs 大目录）。附 token-service `RS256KeyPair::set_key_id` → `with_key_id`（jwt-simple 0.12.16 trait `RSAKeyPairLike` 未实现 `set_key_id`，之前 CI 跑 `--all-targets` 隐藏了 lib 编译错；改 `--lib` 后暴露）。
- [x] **5B-fu6** Principal extractor 响应 body 不泄露 JWT 内部错误（commits `b54de25` + `1f357c9` + `196817f` + `0fc05d6`；前置 `99fc5f9` 加 warn 日志）。6 类错误路径回归测试守住 body 为空。详见 `done.md` Phase 5-B 段。Spec: `docs/superpowers/specs/2026-07-08-5b-fu6-jwt-error-leak-design.md` / Plan: `docs/superpowers/plans/2026-07-08-5b-fu6-jwt-error-leak.md`。

### Phase 5 — 服务间安全 & 韧性（P1）

> **已拆分为两个独立子项目（2026-06-28 brainstorm + plan-review，均已 CLOSED）：**
>
> - **A = mTLS / 服务网格**：见 Phase 5-A / 5-A-2 spec。
> - **B = 韧性三件套**：见
>   `docs/superpowers/specs/2026-06-28-phase5-resilience-triad-design.md`。

- [x] **A — mTLS / 服务网格**（spec + plan + eng-review 已就绪，**已完成**）
  > Spec: `docs/superpowers/specs/2026-07-03-phase5a-consul-connect-mtls-design.md`
  > Plan: `docs/superpowers/plans/2026-07-03-phase5a-consul-connect-mtls.md`
  > 选型：Consul Connect（复用现有 Consul 1.15，完整 mesh：mTLS + 意图 + 可观测性）
  > Review 决策：1A gateway permissive / 3A internal 端点 query param tenant_id / 4A sidecar 超时兜底 / 5A consul-init 容器
  - [x] 实施 Phase 5-A plan（5 个 Task，commits `f5887b1` / `6847007` / `947759f` / `89294b2` / `fa17c14` / `90289a2` / `1f06dd5` / `d8da3bf`）
  - [x] ~~**5A-fu1** mesh metrics Grafana 面板~~ — **已撤销**（Phase 5-A-2 撤 Consul Connect，无 sidecar/Consul Dataplane，metrics 源不存在）

- [x] **B — 韧性三件套**（**已完成并合并** — 核心 ResilientClient + 扩面 B-fu1）
  - [x] 在 `libs/digua_common-http` 提供通用 `ResilientClient`（包 reqwest）：超时 + 重试 + 熔断
  - [x] 本期接 gateway `forward_to_service`：每下游 service 独立 breaker，状态经 `ResilienceObserver` → Prometheus
  - [x] 删除并重写未使用的韧性脚手架（axum/benchmarks/rate_limit/middleware/service_discovery/circuit_breaker/retry/config）
  - [x] **B-fu1** 韧性三件套扩面到其余服务间直调（token→user、各服务 `digua_common-http::HttpClient` / `digua_common::http::client()` 调用点）。
        token-service 的 `fetch_json` 已从裸 hyper 切到 `ResilientClient`（commit `a8cc412`）。
        当前仅 gateway outbound + token-service → iam 两条路径受保护；其余服务间无 HTTP 直调（token/iam/notification/audit 通过事件总线通信，config/file/prue 无出站调用）。

- [x] ~~**C — 基础设施层 TLS**~~ — **延后至 Roadmap 2.0**（1.0 内不增加安全相关功能；范围：RabbitMQ AMQP / MySQL / Redis 连接启用 TLS，与 HTTP mesh mTLS 形成全链路加密。当前状态：Phase 5-A 只覆盖服务间 HTTP 流量 mTLS；AMQP/MySQL/Redis 仍为 docker network 明文）
 
**Phase 5 = ✅ CLOSED**（5-A mTLS 撤并至 registrator + 5-B JWT 验签 + service-jwt + 通知事件化；5-C TLS 推 R2.0）

### Phase 6-A — Trace 链闭环 + 事件总线生产化（P0，2026-06-28 锁定，2026-06-29 全部完成并合并）

> 来源：plan review 2026-06-28（SCOPE REDUCTION → Phase 6-A 四件套）。
> 把 trace 链 + outbox + consumer 错误分级三处 Phase 4 broad-review 遗留 + Phase 6 OTel 全覆盖
> 收敛为一个冲刺，作为 Phase 5-A mTLS / Phase 7 K3s 化的前置。
>
> 五个评审决策已 bake：
>
> - 1A bounded redelivery（redelivered flag + republish-and-ack 模式）
> - 2A enqueue 内自动注入 trace_id（callsite 零改动）
> - 3A testcontainers 真 broker 集成测试
> - 4B archive 分批 1000 行/批
> - 4-Q2A envelope x-redelivery-count header 阈值 3 兜底
>
> 0-Q1 A 扩 HandlerError 双分支 · 0-Q2 A 提取 tracing_mw 到 digua_common-telemetry-axum

- [x] **T1 — Envelope.trace_id producer 自动注入**（覆盖 I4）
      spec: `docs/superpowers/specs/2026-06-28-phase6a-t1-trace-id-injection-design.md`
      plan: `docs/superpowers/plans/2026-06-28-phase6a-t1-trace-id-injection.md`
  - [x] `libs/digua_common-telemetry/src/trace_id.rs` + `outbox.rs` 在 enqueue 内自动注入（commits `798683b`, `4a4ff51`）
  - [x] 6 处 callsite 改 `let mut envelope` + `&mut envelope`（mechanical） + 4 处 digua_common-event tests（commit `5ac963b`）
  - [x] 单测 3 个（fill / respect / no-span） + integration test + e2e script + `test-case/61-trace-id-injection.md`（commits `3d6e86e`, `b721799`）

- [x] **T2 — OTel 全覆盖 + tracing_mw 提取共用**
      spec: `docs/superpowers/specs/2026-06-28-phase6a-t2-otel-coverage-design.md`
      plan: `docs/superpowers/plans/2026-06-28-phase6a-t2-otel-coverage.md`
      merge commit: `0a832c7`
  - [x] 提取 4 份 copy `tracing_mw.rs` → `libs/digua_common-telemetry/src/middleware.rs`（commit `ac5b614`）
  - [x] 删 4 份 copy + 改 4 个服务 import（commit `6e62091`）
  - [x] audit-service / config-service / file-service / prue_backend 接 `init_tracing` + middleware（commits `6f7cae1`, `5cdc1d8`）
  - [x] audit consumer 用 envelope.trace_id 设 span parent（参考实现，`consumer_span.rs`）
  - [x] README OTel 表 4 个 `—` 改 `✅` + `test-case/62-otel-cross-service-chain.md`（commit `185de1a`）
  - [x] **T2-fix** 拆出 `libs/digua_common-telemetry-axum/` 让 CLI binary 不再透传 axum（commit `e511299`）
  - [x] **T2-fu2** `docs-site/USER_MANUAL.md` / `docs-site/ai-context.md` / `docs-site/guides/observability.md` 删 `tracing_mw` 旧说明

- [x] **T3 — Consumer transient/permanent 分级 + bounded redelivery**（覆盖 I1）
      spec: `docs/superpowers/specs/2026-06-28-phase6a-t3-consumer-error-classes-design.md`
      plan: `docs/superpowers/plans/2026-06-28-phase6a-t3-consumer-error-classes.md`
      merge commit: `10e4b07`
  - [x] `HandlerError` 扩 `Transient(String)` / `Permanent(String)` 两分支（commit `6467b5a`）
  - [x] `consumer.rs` delivery 循环按分类决策；transient 走 republish-and-ack + `x-redelivery-count` 阈值 3 + `confirm_select`（commit `49badff`）
  - [x] 7 unit tests + 5 testcontainers 集成测（commits `4ac6741`, `76319af`）
  - [x] 4 个现有 handler 同 PR 审查改造（commit `57c075d`）
  - [x] `test-case/63-consumer-error-classes.md`（commit `18b55af`）
  - [x] **T3-fu3** testcontainers 测的 `sleep(4s)` 改 poll-with-deadline（commit `consumer_redelivery.rs` poll_dlq helper）
  - [x] **T3-fu4** api-gateway perm_consumer 行为变更发布说明（写入 `services/api-gateway/AGENTS.md`）

- [x] **T4 — Outbox 24h 归档**（覆盖 I6）
      spec: `docs/superpowers/specs/2026-06-28-phase6a-t4-outbox-archive-design.md`
      plan: `docs/superpowers/plans/2026-06-28-phase6a-t4-outbox-archive.md`
      merge commit: `5389b24`
  - [x] 迁移 V002\_\_outbox_archive.sql 新建 `iam_outbox_events_archive`（commit `471184b`）
  - [x] `services/iam-service/src/outbox_archive.rs` archive_loop（`BETWEEN min..max` + 二次 WHERE）+ purge_loop（30d）+ `GET_LOCK` 多副本互斥（commit `db0d77c`）
  - [x] `outbox-publisher` binary 起 archive tokio task（commit `80076b2`）
  - [x] testcontainers 7 个 + `test-case/64-outbox-archive.md`（commit `2e27513`）
  - [x] **T4-fu1** `docs-site/USER_MANUAL.md` outbox section 加 archive 30d 一行（spec §5.5 要求，plan 未列，implementer acknowledge）

### Phase 6-A 后续项（2026-06-29 plan-review 重排序，按优先级）

> 评审结论已 bake：
>
> - **T3-fu1** → `libs/digua_common-event/src/consumer.rs` 注册 `Basic.Return` 回调 + oneshot channel，unroutable 时返回 Err（不 ack 原 delivery）
> - **T3-fu2** → 实现 `republish_confirm_nack_does_not_ack_original` 集成测（`channel.close()` 模拟 republish 失败）
> - **T2-fu1** → `span_from_envelope_trace_id` 从 `audit-service/src/consumer_span.rs` 搬到 `libs/digua_common-event/src/span.rs`，notification/gateway 统一调用
> - **T2-fu3** → 新建 `services/iam-outbox-publisher/` 独立 crate（仅依赖 sqlx + digua_common-event + digua_common-telemetry + iam-service lib），不再拖 axum
> - **T2-fu4 / T4-fu2** → 合并：`outbox_archive.rs` 加 `iam_outbox_archived_total` + `iam_outbox_purged_total` 两个 IntCounter（复用 `OnceLock` 模式）

#### P0 — 正确性修复

- [x] **T3-fu1** `Basic.Return` 静默丢消息（评审 #2，正确性 bug）
      `libs/digua_common-event/src/consumer.rs` 改用 `wait_for_confirms()` 同时等待 publisher confirm 并收集 `Basic.Return` 消息；unroutable 时返回 Err 让外层不 ack。
      修复 commit: `consumer.rs` `republish_with_incremented_count` 函数重构。

- [x] **T3-fu2** `republish_confirm_nack_does_not_ack_original` 集成测（评审 #3，核心正确性未实测）
      新增 `republish_confirm_nack_does_not_ack_original` 集成测：unbind routing-key → handler Transient → republish 遇 Basic.Return → 原 delivery 未 ack → 消费者重连后消息重新可见。
      新增 `republish_fails_on_nonexistent_exchange` 直接函数测试。
      `libs/digua_common-event/tests/consumer_redelivery.rs`。

#### P1 — 架构改进

- [x] **T2-fu1** notification / gateway perm_consumer 设 span parent（评审 #1）
      `span_from_envelope_trace_id` 从 `audit-service/src/consumer_span.rs` 搬到 `libs/digua_common-event/src/span.rs`；
      notification durable/ephemeral consumer + gateway perm_consumer 三处调用 `.instrument(span)`。
      删 `services/audit-service/src/consumer_span.rs`。

- [x] **T2-fu3** outbox-publisher 拆为独立 crate（评审 #4）
      新建 `services/iam-outbox-publisher/` 独立 crate，仅依赖 `sqlx` + `digua_common-event` + `digua_common-telemetry` + `serde_yaml`；
      内联 `load_config` YAML 解析（不再依赖 `iam-service` 库，避免拖入 axum）。
      删 `services/iam-service/Cargo.toml` 中 `[[bin]] outbox-publisher` 及旧二进制文件。
      更新 `services/iam-service/Dockerfile` 构建命令。

#### P2 — 可观测性

- [ ] **指标** `iam_outbox_archived_total` + `iam_outbox_purged_total` IntCounter（合并 T2-fu4 + T4-fu2）
      **推迟**：outbox-publisher 现为独立 CLI binary，无 `/metrics` HTTP 端点，prometheus counter 注册了也无法被抓取。spec 标注"标可选"。
      若未来需要，可在 binary 内嵌一个极简 `tokio::net::TcpListener` 暴露 `/metrics`，或把指标挪到 iam-service 的 `/metrics` 端点通过共享 DB 读取。

#### P3 — 杂项

- [x] **T2-fu2** 删 docs 中 `tracing_mw` 旧说明（`USER_MANUAL.md`、`ai-context.md`、`observability.md`）
- [x] **T3-fu3** testcontainers `sleep()` → poll-with-deadline（`consumer_redelivery.rs` 多处）
      新增 `poll_dlq()` 辅助函数；原有 `sleep(2s/3s/4s)` 全部替换为轮询或计数器轮询，避免慢 CI flake。
- [x] **T3-fu4** api-gateway perm_consumer 行为变更发布说明
      写入 `services/api-gateway/AGENTS.md` "令牌失效 perm_epoch" 段。
- [x] **T4-fu1** `docs-site/USER_MANUAL.md` Troubleshooting 表格新增 outbox archive 30d 说明行

### Phase 6-A 总结

- 4 份 spec + 4 份 plan + 4 份进度 ledger（`.superpowers/sdd/progress-phase6a.md`）
- 21 个 feature commit + 4 个 merge commit 推上 main
- 全部 task 实施 + review Approved
- 已知 follow-up（T2-fu1~4, T3-fu1~4, T4-fu1~2）已挂 TODO，单独迭代

### Phase 6-B — Config-Service 自举（Dogfooding，P1）

> Spec: `docs/superpowers/specs/2026-06-29-phase6b-config-service-dogfood-design.md`
> Plan: `docs/superpowers/plans/2026-06-29-phase6b-config-service-dogfood.md`
> 策略：token-service 作为第一个 dogfood 服务；其余后续。

- [x] **T1 — `libs/digua_common-config` 加 `ConfigServiceProvider`**
      merge: `9aa2a24`
  - [x] 新增 `config-service` feature + `ConfigServiceProvider`（HTTP 拉 `/config/export/:ns`）（commit `2d43c6d`）
  - [x] wiremock 3 个单元测试（200-ok / 404-err / invalid-json-err）

- [x] **T2 — `Loader` 集成 provider + 热更新 watch**
  - [x] `with_config_service(url, ns)` builder（commit `9e5894e`）
  - [x] `load()` async；file base → config-service override → env override
  - [x] `watch_config()` 后台 30s 轮询 + `Config::builder` deserialize + diff + `HotConfig::update`

- [x] **T3 — token-service 第一个接入**
  - [x] `Cargo.toml` 开 `config-service` feature（commit `900c276`）
  - [x] `main.rs` 用 `Loader::new("TOKEN").with_config_service(...).with_file(...).load().await` 替代 yaml
  - [x] `test-case/65-config-service-dogfood.md` 手工验收

### Phase 7 — 生产级部署（P10，降级）

> **2026-07-19 决策**：当前无客户使用 K8s，Phase 7 从 P2 降级至 P10。
> 维持 docker-compose 作为唯一部署方案，等出现 K8s 部署需求时再启动。

- [ ] **K3s / Kubernetes 化**
  - [ ] 服务 chart / manifest 编写（替代 `docker-compose` 生产用途）
  - [ ] 三级探针：startup / liveness / readiness
  - [ ] ConfigMap + Secret + RBAC 集成
  - [ ] HPA 基于 CPU + 自定义 metrics（QPS / queue depth）

- [ ] **灰度发布升级**
  - [ ] 引入 Argo Rollouts 或 Flagger，替换 Consul tag + Nginx split_clients
  - [ ] 指标驱动的金丝雀（错误率 / P99 自动回滚）

### Phase 8 — 多租户基础（P2，按产品形态择机）

> 设计决策（brainstorm 2026-07-01）：共享 MySQL + tenant_id 列 / 一用户一租户 / 自助注册 / JWT `tid` claim 唯一来源 / BIGINT 自增 / permissions 全局 + roles 租户独有。
> 拆 3 个子 spec：8.1 骨架 / 8.2 表迁移 / 8.3 全面贯通。

#### Phase 8.1 — 骨架（已完成并合并）

> Spec: `docs/superpowers/specs/2026-07-01-phase8.1-multi-tenancy-skeleton-design.md`
> Plan: `docs/superpowers/plans/2026-07-01-phase8.1-multi-tenancy-skeleton.md`

- [x] V003 迁移：`iam_tenants` 表 + default seed + `iam_users.tenant_id`
- [x] `libs/jwt-ctx::Principal.tenant_id` + tid claim 解析
- [x] `libs/jwt-ctx::TenantScope` extractor
- [x] `libs/digua_common-db::WithTenantScope` helper
- [x] `services/iam-service::token_svc::issue_access_token` 加 tenant_id 参数 + JWT `tid` claim
- [x] iam-service 5 处 Envelope::new callsite 注入 tenant_id
- [x] `contracts/events.yaml` 加 `iam.tenant.created` + codegen
- [x] `POST /iam/tenants` 注册接口 + 同事务建 admin + 发事件
- [x] `LoginRequest.tenant_slug` + login tenant-scoped lookup
- [x] testcontainers 集成测 + 全量 docker 单测回归 + 手工验收 `test-case/81-tenant-registration.md`

#### Phase 8.2 — 业务表 tenant_id 迁移（**已完成并合并**）

> 范围：iam_roles / iam_permissions / iam_announcements / iam_outbox_events / audit_* / ntf_* / cfg_* / file_* / prue_* 全部加 tenant_id 列与索引，所有 SELECT/INSERT/UPDATE 改为 tenant-scoped，default tenant backfill。
> 阻塞条件：8.1 已上线 + 至少 1 个外部租户实际接入或临近接入。
> Plan: `docs/superpowers/plans/2026-07-03-phase8.2-iam-tenant-business-tables-plan.md`

- [x] brainstorm 8.2 spec：表清单、迁移顺序、索引策略、跨服务 SELECT 修改清单
- [x] 迁移 V004（按服务）：每个服务各自 ALTER 业务表加 tenant_id
- [x] sqlx 查询点全量改造（参考 `WithTenantScope::and_tenant`）
- [x] roles 改为 tenant 内（移除全局 tenant_admin seed，注册流程改为复制角色模板到 tenant）
- [x] envelope.tenant_id consumer 侧 `parse::<i64>()` 注入 audit/ntf insert
- [x] Task 7: 回归 + 手工验收 + TODO 勾选（commit `4dc2046`）
- [x] `get_claims` tenant 过滤补完（Phase 8.3 scope，commit `59b76a7`）

#### Phase 8.3 — 全面贯通 + 跨租户检测（已完成并合并）

> 范围：gateway 跨租户访问检测、JWT 缺 tid 收紧为 401、`scripts/check-tenant-scope.js` lint、platform-admin `cross_tenant` flag、e2e 跨租户黑名单。
> 阻塞条件：8.2 已上线。

- [x] brainstorm 8.3 spec（见 `docs/superpowers/specs/2026-07-01-phase8.1-multi-tenancy-skeleton-design.md` §3.6）
- [x] gateway 中间件：对比 `Principal.tenant_id` 与请求中可能携带的 tenant_id（URL/Body 路径参数）一致性（`check-tenant-scope.js` + TenantScope extractor）
- [x] `Principal::from_jwt_payload` 缺 `tid` → 401（`MissingTenantId` error in `libs/jwt-ctx/src/principal.rs:33`）
- [x] `scripts/check-tenant-scope.js`：handler 拿 TenantScope 但 SQL 无 tenant_id 时 lint 报警
- [x] platform-admin 引入 `cross_tenant: bool` 路径，独立于 default tenant（V005 migration + `token_svc::issue_access_token` `ct` claim）
- [x] e2e 跨租户访问黑名单（`tests/e2e/api/cases/83-cross-tenant.yaml` + `test-case/83-cross-tenant.md`）
- [x] `UserService::get_claims`（`/iam/internal/users/:id/claims`）加 tenant_id 过滤（`user_svc.rs:416` `AND tenant_id = ?`，commit `59b76a7`）

#### Phase 8.3-fu1 — iam-service 点查询 TenantScope 迁移（已完成并合并）

> Plan: `docs/superpowers/plans/2026-07-04-phase8.3-fu1-iam-point-query-migration.md`
> Spec: `docs/superpowers/specs/2026-07-04-phase8.3-fu1-iam-point-query-migration-design.md`

- [x] `libs/digua_common-db::WithTenantScope` 加 `where_id(id)` helper（commits `697db59`）
- [x] `user_svc.rs` 9 方法签名 + 11 SQL 迁 `scope: &TenantScope` + QueryBuilder + `and_tenant`/`where_id`（commit `8d0fdd6`）
- [x] `role_svc.rs` 7 方法 + 7 SQL 同上（commit `b177040`）
- [x] `org_svc.rs` 8 方法 + 7 SQL 同上（commit `2e8739d`）
- [x] `routes/users.rs` 8 handler 签名改 `TenantScope` extractor（commit `47bc4cb`）
- [x] `routes/roles.rs` 7 handler 同上（同 commit）
- [x] `routes/org_units.rs` 6 handler 同上（同 commit）
- [x] 集成测试 `platform_admin_point_query_cross_tenant`：HTTP GET `/iam/users/{acme_user_id}` with platform_admin JWT → 200 + body 命中 acme user（commit follow-up）
- [x] e2e YAML `cross_tenant_point_query_200`（CT-011）纳入 83-cross-tenant.yaml（同 commit）
- [x] 已知 lint 误报：`scripts/check-tenant-scope.js` 仍把 handler 报为「SQL 无 tenant_id 过滤」——脚本不会跟随 service 调用，加 `delegated` 豁免即可，跟踪在 8.3-fu2 工具链一并升级

#### Phase 8.3-fu2 — ntf / cfg / file 点查询迁移 + check-tenant-scope 启发式升级（**已完成并合并**）

> 阻塞：8.3-fu1 已上线，且至少一个外部租户实际接入 ntf / cfg / file。
> Plan: `docs/superpowers/plans/2026-07-04-phase8.3-fu2-ntf-cfg-file-tenant-hardening.md`
> Spec: `docs/superpowers/specs/2026-07-04-phase8.3-fu2-ntf-cfg-file-tenant-hardening-design.md`

- [x] notification-service：8 处 point queries（announcement / role / dept）迁 `scope: &TenantScope` + `and_tenant`/`where_id`（commits `d2b91fb..6d816d4`）
- [x] config-service：5 处 point queries（config_item / namespace）同上（commits `3ad1163..a77629c`，含 V002 迁移 + 引入 Principal auth + drop sqlite + namespace ownership 校验）
- [x] file-service：8 处 point queries（file_object / permission）同上（commits `48f91a2..400583f`，含 VARCHAR→BIGINT 迁移 + drop sqlite）
- [x] `scripts/check-tenant-scope.js` 启发式升级 — **未升级，仅沿用 8.3-fu1 delegated 豁免**：cfg/file/ntf 的 service.method 已迁 QueryBuilder + and_tenant/where_id；handler 仍 delegate 给 service，加 `// check-tenant-scope: delegated` 注释通过 lint。后续若要跟随 callee body 进行 callee-level 校验，需 lib 跟踪升级（独立 follow-up）
- [x] e2e 黑名单扩展（`tests/e2e/api/cases/83-cross-tenant.yaml` CT-012~CT-020）：`cfg_cross_tenant_isolated` / `file_cross_tenant_isolated` / `ntf_publish_cross_tenant` 三组 × 3 步（commit `8ff62bb`）

---

## Phase 8 总结

- 8.1 骨架：JWT tid claim + TenantScope extractor + 共享 MySQL + 自助注册 — 完成
- 8.2 业务表 tenant_id 迁移：iam/audit/ntf/cfg/file/prue 全表加 tenant_id — 完成
- 8.3 全面贯通：gateway 中间件 + 缺 tid 401 + cross_tenant flag + e2e 黑名单 — 完成
- 8.3-fu1 iam 点查询：where_id + and_tenant + tenant-scope extractor 全面迁移 — 完成
- 8.3-fu2 ntf/cfg/file 点查询：同 pattern + cfg auth 补全 + file V002 类型对齐 + cfg/file drop sqlite — 完成

**Phase 8 = ✅ CLOSED**

### Phase 9 — Java 8 集成示例 + Audit SDK（P3，独立交付，**已完成并合并**）

> Spec: `docs/superpowers/specs/2026-06-29-java8-webapi-demo-design.md`（Status: **Approved**）
> Plan: `docs/superpowers/plans/2026-06-29-java8-webapi-demo-plan.md`
> 范围：`sdk/java/audit-service-sdk`（手写 SDK 补缺位）+ `examples/java8-webapi/`（Spring Boot 2.1.18.RELEASE + Java 8 Demo，覆盖 6 类能力 + 1 业务工单示例）。
> 完成：17 个 commit（2026-06-29 ~ 2026-06-30），30 个 unit/MockMvc 测试全过（audit-service-sdk 4 + demo-shared 4 + demo-webapp 22），`mvn -Pjava8 install` 成功。

- [x] `sdk/java/audit-service-sdk` 骨架 + 模型 + 异常 + 入口（commit `bc82917`）
- [x] `examples/java8-webapi` 父 POM + 目录骨架（commit `1850518`）
- [x] `examples/java8-webapi` 配置内部 Maven 仓库 Reposilite（commit `a1f402b`）
- [x] `demo-shared` 模块 ApiResponse/ErrorCode/PageReq（commit `b32ee1c`，`request_id` snake_case 修复 `e9de036`）
- [x] `demo-webapp` 基础（POM + 启动 + 配置 + Bean + 全局异常）（commit `c74ecde`）
- [x] Task 实体 + TaskStatus 枚举 + TaskRepository（commit `bbb24b8`）
- [x] TaskService 业务主轴（CRUD + 状态机）（commit `a5d7066`）
- [x] JwtAuthInterceptor + @RequiresPermission AOP（commit `36294c8`）
- [x] TaskController + 3 MockMvc tests（commit `a153977`）
- [x] AuthDemoController (me/jwks/refresh/revoke) + 2 tests（commit `c51b03d`）
- [x] PermissionDemoController (public/internal/resource 3 场景) + 3 tests（commit `b0fd7fb`）
- [x] AuditPublisher + AuditDemoController (3 端点) + 2 tests（commit `005c209`，TaskService 接入 `187f8c1`）
- [x] ConfigPoller 5s 轮询 + ConfigDemoController (2 端点) + 1 test（commit `5cfb75d`）
- [x] NotificationDemoController (task-done) + 1 test（commit `ac2039b`，ObjectMapper 化 `73b8c49`）
- [x] FileDemoController (upload/download) + 2 tests（commit `0ee5623`，ObjectMapper 化 `73b8c49`）
- [x] TaskService + AuditPublisher 接入（commit `187f8c1`）
- [x] Task read endpoints 强制 TASK_VIEW 权限 + 1 test（commit `72261ba`）
- [x] `scripts/demo.sh`（15 个 curl）+ README 跑通教程（commit `0c9fde7`）
- [x] spec 状态从 Draft → Review 推到 Approved，TODO.md 收尾条目（commit `本次`）

后续（spec §10 显式排除项，本次未做）：
- audit-service-sdk 发布到内网 Maven 仓库（当前仅 install 到 `~/.m2`）
- demo-webapp 加 Docker 镜像
- token 自动刷新拦截器
- 配置中心改 push / long-poll
- audit 上报改异步队列
- demo 集成 e2e（`tests/e2e` 加 java8-webapi 跑通套件）

### Phase 9.5 — Rust 多语言 SDK 套件（P3，与 Java SDK 对齐，**已完成并合并**）

> 与 Phase 9 Java 8 SDK + Demo 对偶的 Rust 侧补齐。范围：把 Rust 侧的 `sdk/rust/kudi-http-client` 升级为共享底座，并补齐 6 个 service 业务 SDK（与 Java `sdk/java/{iam,token,audit,file,notification,config}-service-sdk` 1:1 对齐），让外部 Rust 应用能用与 Java 等价的 SDK 调平台公网 API。
>
> 设计原则：
> - 端点路径一律以 `services/<svc>/openapi.yaml`（utoipa 自动 dump 的 single source of truth）为事实源——Java SDK 部分路径已过期（典型如 `/v1/audit/...` 在现网服务中不存在），Rust 端不照搬。
> - Bearer 模式：所有 service SDK 用 `Fn() -> Option<String> + Send + Sync` closure 持有 user JWT（与 Java `Supplier<String>` 对齐），调用方负责 token 生命周期。
> - 错误模型：5 种 `SdkError` variant 严格按 `docs/superpowers/specs/2026-06-08-multi-lang-sdk-design.md` §9（`Auth` / `Validation` / `RateLimit` / `Server` / `Network`）。
> - 内部 service 不使用 SDK——Rust service 间通信走 `libs/digua_common-http::ResilientClient` + `digua_common_jwt::ServiceJwtCache`（service-jwt + Consul/internal 端点），与 SDK 的 client_credentials + Bearer passthrough 是不同的信任域（详见 plan §0）。
>
> Plan（本地）：`.omo/plans/2026-07-17-rust-service-sdk.md`（10 节：架构、crate 布局、命名、错误、测试、CI、风险、DoD、显式 out-of-scope）
> 范围外的明确不做项（plan §9）：为 prue_backend / api-gateway 加 SDK；改 Rust service 内部通信；改 `dump-openapi-specs.sh` / `check-spec.js`；发内部 cargo registry；做 e2e；拆 user/client 两个 crate（用 Builder 切换场景）。

- [x] `kudi-http-client` 增强为共享底座（新增 `SdkError` alias / `Builder::base_url` + `with_base_url` / `base_url()` getter / `force_refresh()` + README「作为 SDK 共享底座」一节 + `tests/sdk_base_test.rs` 3 个新测试）
- [x] `audit-service-sdk`（样板，6 方法：query_login_logs / query_operation_logs / trigger_cleanup / get_retention_policy / update_retention_policy；6 wiremock + 1 doctest）
- [x] `iam-service-sdk`（11 方法：login / refresh / logout / change_password + list_users / get_user / create_user / update_user + list_roles / get_role / create_role / update_role + list_permissions；11 wiremock + 1 doctest）
- [x] `token-service-sdk`（5 方法：access / refresh / revoke / list_api_keys / create_api_key；6 wiremock）
- [x] `file-service-sdk`（3 方法：create_presign / list_files / get_file_info；4 wiremock；upload + download 走 presign URL，由调用方直接 PUT/GET）
- [x] `notification-service-sdk`（5 方法：list_announcements / get_announcement / create_announcement / publish / unread_count；5 wiremock；WS 推送不包装，调用方用 tungstenite 客户端直连 `/notification/ws`）
- [x] `config-service-sdk`（4 方法：list_namespaces / create_namespace / list_items / create_item；5 wiremock；SSE watch 不包装，调用方用 eventsource-client）
- [x] 顶层 `sdk/rust/README.md`（crate 矩阵 + install/usage/扩展指南 + 错误模型 + 与 Java SDK 的差异说明 + 扩展模板）
- [x] 根 `Cargo.toml` workspace 注册 6 个新 crate + `sdk/rust/kudi-http-client` 已在内
- [x] 全部 SDK 集成 `cargo build -p` + `cargo test -p` 通过（46 单元 + 2 doctest = 48 测试全过，0 fail；目标 `sdk/rust/kudi-http-client` + 6 个 service SDK）
- [x] `services/<svc>/openapi.yaml` 重 dump 验证零 drift（25 个 SDK endpoint ↔ yaml 全部 OK；dump 脚本 fallback 成功；无 yaml 改动）
- [x] 现有 service 测试不破坏（未触碰 `services/*`、`libs/*`、`sdk/java/*`、`sdk/openapi/*`、根 `Cargo.toml` workspace 顺序）

#### 未做项（plan §9 已显式排除 / 留待后续）

- 每个 service SDK 还有未包装的端点（详见各 crate README 的「Not wrapped」段）：iam（org-units、sessions、tenants、role/permission attach、user roles assign）；token（user-jwt、oauth-clients admin、reset-secret）；audit（compliance export csv/json、retention stage/cleanup）；file（upload multipart、download bytes）；notification（mark_read / 我的通知列表）；config（namespace/{id} get/put、item/{id} get/put、watch SSE、export/import）。补全按各 SDK 现有模式扩展即可。
- `scripts/check-rust-sdks.sh`（独立 build/test 脚本）：当前用直接 `cargo build/test` 命令验证，nice-to-have 不是 must；后续 CI gate 接入时一并补。
- 内网 cargo registry 发布：项目当前没有内部 cargo registry，文档说明先 `path = "..."` 依赖，后续补 `cargo login` + `~/.cargo/config`。
- CI 门禁接入：未改 `.gitlab-ci.yml`；文档说明可后续通过现有 `run-tests-docker.js` 工作流接入。
- DELETE method / multipart upload / binary download：当前 `kudi-http-client` 的 `HttpTransport` trait 只暴露 GET / POST（post_json / post_form），且 `HttpResponse.body` 是 text-only。如需支持，给 `KudiWebClient` 加 `delete` 方法 + `HttpTransport` 加 `delete_json`/`post_multipart`，并把 `HttpResponse.body` 改成 `Vec<u8>`（破坏向后兼容，等下一次大版本再考虑）。
- Momus plan review：subagent 30 分钟 timeout 未返回评审意见，本工作全部决策由 implementer 基于已读代码 + 项目规则自行判断（详见 plan §0-§9）。

### Phase 10 — 小问题清单（P3，零散收尾）

- [x] `prue_backend` 评估是否去掉 SQLite 分支，统一 MySQL（SQLite 仅留 dev 模式） — **决策：保留 SQLite**（spec `2026-07-08-prue-backend-sqlite-decision-design.md`）。仅 1 个 migration，翻译成本可接受；dev/CI 零依赖启动价值更高。配套加 `scripts/check-prue-migrations-parity.sh` CI gate 防 drift。
- [x] `events.yaml` — **Phase 4 已落地**（首批三事件 + Phase 5B-fu3 三事件 `iam.user.role.{assigned,unassigned}` + `iam.user.dept.changed` 已 codegen + producer/consumer 闭环）
- [x] 健康检查补齐三级探针 — 7 个 Rust 服务 Dockerfile 已补 HEALTHCHECK（commit `8eec3d4`），参数：`--interval=15s --timeout=3s --retries=5 --start-period=20s`
  - [x] `pom.xml` 补 `spring-boot-starter-test` test scope
  - [x] `config/application.yml` 移除死代码 datasource 段（compose 注 `SPRING_DATASOURCE_*`）
  - [x] `Dockerfile` 的 maven-settings.xml 改为 `ARG MAVEN_SETTINGS` build-arg，移除 `-Dmaven.test.skip`
- 数据库 schema 当前版本/历史查询端点（`/internal/migrations/state`） — **G3 7/7 端点接入完成（待集成测试 + 文档同步）**
  - [x] spec `docs/superpowers/specs/2026-07-08-migrations-state-endpoint-design.md`
  - [x] `libs/digua_common-db::MigrationState` trait + `build_response`（commit `61d4b0c`）
  - [x] iam-service 接入（commit `bcb5bcf`，5 migrations）
  - [x] notification-service 接入（commit `8706375`，3 migrations）
  - [x] token-service 接入（commit `2b7207d`，1 migration）
  - [x] audit-service 接入（commit `b5b59ea`，1 migration）
  - [x] config-service 接入（commit `25a6bf6`，2 migrations）
  - [x] file-service 接入（commit `25a6bf6`，2 migrations）
  - [x] prue_backend 接入（commit `8a19729`，1 migration × 2 dialects）
  - [x] 集成测试 + 文档同步 — CI job `e2e_migration_state_test` 已接入（`.gitlab-ci.yml`）；AGENTS.md 7/7 已有端点条目；`docs-site/guides/migrations-state.md` 已完整
- [x] 密钥轮换机制 SOP（JWT 私钥、DB 密码、API key） — runbook `docs-site/runbooks/secret-rotation.md`。覆盖：① JWT 私钥（含 kid + JWKS 路线图），② MySQL root 密码，③ Redis 密码；含手动 SOP 命令与未来自动化路线。
- [x] gateway 限流统一走 Redis，淘汰 in-memory 实现避免双实现困惑（commit `ddf44a0`）
- [ ] 评估自研 router 替换为 Envoy / Pingora 的成本/收益
- [x] **iam-service `dump-openapi` binary 栈溢出**（阻塞 openapi.yaml 自动同步）
  - **根因**：`services/iam-service/src/dto/org_unit.rs:OrgUnitResponse` 有 `children: Vec<OrgUnitResponse>` 自引用字段，utoipa `ToSchema` derive 展开时无限递归内联，导致 `openapi()` / `to_yaml()` / Drop 均在主线程栈溢出。
  - **修复**：`#[schema(no_recursion)]` 属性宏加到 `children` 字段，告知 utoipa 用 `$ref` 引用而非内联展开。这是 utoipa 官方处理自引用 schema 的标准方式，运行时的 JSON 序列化不受影响。
  - **验证**：`dump-openapi` exit 0，1794 行完整写出，`openapi.yaml` 回归自动生成（不再需手工同步）。
  - **commit**：`1157406`
- [x] **OpenAPI registration sweep** — 2026-07-07 补齐（commit `a2cd626`：`org_units::*` 6 handler + `permissions::*` 5 handler + DTO ToSchema/IntoParams + openapi.yml 手工补齐；dump-openapi binary stack overflow pre-existing）
- [ ] **i18n 命名空间修复后续**（commit `20488c2` 把 359 keys 从 `buttons:` 迁到 `menus:` namespace，修复 422 处原始 key 暴露；但扫到 3 处**真正**没翻译的 key 缺失，不是 namespace 错位）：
  - `frontend/src/router/modules/account.ts` 用 `t('menus.accountProfile')` / `t('menus.accountSecurity')` / `t('menus.accountSettings')`，但 yaml 中**只**有 `pureProfile` / `pureSecurityLog` / `pureAccountSettings`（带 `pure` 前缀，在 `menus:` line 229/301/2）—— key 命名不一致，fallback 显示原始字符串。
  - **修复方案**（二选一，本 session 不动）：
    - (A) 在 `menus:` namespace 下新增 3 个 alias key：`accountProfile: 个人信息`、`accountSecurity: 安全日志`、`accountSettings: 账户设置`（zero call-site 改动）
    - (B) 改 `account.ts` 用 `pureProfile` / `pureSecurityLog` / `pureAccountSettings`（zero yaml 改动，但语义稍变：profile 是「个人信息」不是「账户 Profile」）
  - **关联**：本 commit `20488c2` + plan `docs/superpowers/plans/2026-07-10-frontend-i18n-namespace-fix-plan.md` §6「不在范围」。修方案 A 需 spec（新增 i18n key 是 contract 改动），修方案 B 改 router title（影响导航文案）。
  - **已知误报**（无需修）：`t('tableBar.pureCollapse')` / `t('tableBar.pureList')` 在 `RePureTableBar/src/bar.tsx` / `ReVxeTableBar/src/bar.tsx` 用 `tableBar:` 前缀，key 在 `tableBar:` 下**也**有定义（line 710/715）—— 工作正常，仅 namespace 用法略不规范。

---

## Roadmap 2.0 — 1.0 之后的延后项

> 决策时间：2026-07-07。本项目 1.0 阶段原则上不增加新的安全相关功能，
> 等 1.0 落地、跑稳一段时间（拿到真实流量和合规要求）后再统一规划。
> 以下两项从 Phase 5 拆出，归入 2.0：

### R2.1 — 基础设施层 TLS（Phase 5-C）

- 范围：RabbitMQ AMQP / MySQL / Redis 连接启用 TLS，与 HTTP mesh mTLS 形成全链路加密
- 当前状态：Phase 5-A mTLS 只覆盖服务间 HTTP 流量；AMQP/MySQL/Redis 仍为 docker network 明文
- 涉及：RabbitMQ/MySQL/Redis 服务端 TLS 证书配置 + 8 个服务连接串改 `amqps://` / `mysql://?tls=1` / `rediss://` + 证书分发
- 阻塞条件：1.0 落地 + 至少 1 个外部租户实际接入并提出合规需求

### R2.2 — TLS termination（Phase 5B-fu4）

- 范围：生产路径需 alb/nginx 终结 TLS（服务间 dev 仍 HTTP）
- spec：`docs/superpowers/specs/2026-07-07-phase5b-jwt-signature-and-service-jwt-design.md` §11（`phase5b-tls-termination.md` 拆分）
- 阻塞条件：同 R2.1

---

## 新员工手册待补充（docs/onboarding/）

以下 TODO 项来自 `docs/onboarding/` 系列文档，需要人工补充：

### 基础设施与权限

- [ ] **内网仓库凭证申请**：Docker Registry (`zhcoder-docker-registry.com:8000`)、Maven (`http://mvn.kudi.repos`) 的账号申请流程 — **待运维补充**
- [ ] **生产数据迁移 SOP**：生产环境数据库迁移的备份、回滚、灰度验证标准流程 — **待运维补充**
- [ ] **集中式日志收集**：生产环境日志收集方案（ELK / Loki / 阿里云 SLS）、查询入口、权限申请 — **待运维补充**

### 团队规范

- [ ] **求救渠道**：技术问题 IM 群号/频道名、on-call 值班表、紧急故障联系人、升级路径 — **待团队负责人补充**
- [ ] **commit message 规范**：是否使用 Conventional Commits、PR 标题格式、是否需关联 issue/ticket — **待规范人补充**

### 技术细节

- [ ] **config-service 使用方式**：如何通过 config-service API 读写配置、各服务如何集成、配置优先级 — **待熟悉 config-service 的开发者补充**
- [ ] **前端登录态刷新机制细节**：Token 刷新触发条件、并发刷新处理、自动登出策略的完整实现 — **待前端负责人确认（目前已基于代码写出基本逻辑，需确认边界情况）**

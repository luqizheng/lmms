# Done — 已完成的 TODO 项

> 来源：TODO.md 历史累积完成项，按 Phase 归档。
> 本文件由 TODO.md 管理，勿手动编辑完成项内容。

---

## Phase 4

### Phase 4.0 — 数据表命名规范统一

- [x] 新建 `contracts/db-conventions.md`，定前缀映射：`iam_/audit_/token_/ntf_/cfg_/file_/prue_`（commit `f3e9aa7`）
- [x] 排查 dev 环境 `login_logs` 撞名实际情况，确定归属（结论：双方各自一份，详见 `.superpowers/sdd/task-0-baseline.md`）
- [x] 各服务 `V00X__rename_tables.sql` 一次性 RENAME 存量表（采用 empty-db-regenerate 策略：把 V001 内表名前缀化）
- [x] sqlx 查询、e2e fixture、测试 setup、文档同步改名
- [x] `scripts/check-db-naming.js` CI gate（commit `143101c`）
- [x] `agents.md` / `services/<svc>/AGENTS.md` 更新规范（commit `2049a33`）
- [x] `coder-migrate-db` skill 模板加前缀默认值

### Phase 4 主任务

- [x] 基础设施：`infrastructure/docker-compose-infra.yml` 加 RabbitMQ + `definitions.json` 预声明 topology（commits `c52f401`, `b0eb5e2`）
- [x] `contracts/events.yaml` 启用首批三事件 + envelope schema（commit `2fe432f`）
- [x] `scripts/contracts-gen.js` 扩展：events.yaml → Rust event struct + routing-key 常量（commit `8e9823b`）
- [x] **`libs/common-event` 完整抽象**：Publisher / Consumer / Envelope / OutboxRepo<DB> trait + lapin RabbitMQ 实现（commits `265adcc` … `e95069f` + minor `8a014de`）
- [x] iam-service 加 `iam_outbox_events` 表 + 业务事务里写 outbox（commits `9f8e06b`, `dc048fa`）
- [x] iam-service 加 `[[bin]] outbox-publisher`（同镜像切 CMD）（commit `7808a7a`）
- [x] audit-service 加 `audit_events` 表 + RabbitMQ consumer + 删除旧 HTTP 写入端点（commits `0749893`, `ed2f63a` + minors `1577ed9`, `7852718`）
- [x] e2e 用例 `scripts/e2e-event-flow.js`（commit `945ff46`）— runner 不支持直接 DB 断言，改 standalone live smoke
- [x] 验收：RabbitMQ 拔掉/恢复、consumer panic → DLQ、Jaeger 端到端 trace 链
  - ✅ Broker-bounce 恢复实测：停 RabbitMQ → 创建 user → 重启 → publisher 10s 内追上（commit `fab9cd2` 修复 C1）
  - ✅ SIGTERM/SIGINT 双处理：docker stop 走 SIGTERM，现在能正确触发 "finish current batch then exit"（commit `fab9cd2` 修复 C2）

### Phase 4.x 后续迭代

- [x] **4.1** notification-service 订阅 `iam.user.created` → 站内信欢迎（commits `94593df`..`38a5267`）
- [x] **4.2** 落地 `iam.login.failed` + 简易风控消费者（commits `492abd8`..`01f7723`）
- [x] **4.3** 落地 `notification.announcement.published`，WebSocket 广播改由事件驱动（commits `c5f423e`..`d9961d0`）
- [x] **4.3-fu2** role/dept 接收人解析落地
- [x] **4.3-fu4** consumer `run()` block-on-disconnect 修复
- [x] **4.4** `iam.permission.changed` 事件 + perm-epoch 令牌失效（commits `512b389`..`4230b06`）
- [x] **4.4-fu1** 新增 `PUT /iam/users/:id/roles` 写端点并接入失效
- [x] **4.4-fu2** actor_id 透传
- [x] **4.4-fu4** `PermissionService::create` 改 INSERT + re-SELECT

### Phase 4 broad review 遗留

- [x] **C1** publisher 无 RabbitMQ reconnect（修复于 `fab9cd2`）
- [x] **C2** ctrl_c 不捕 SIGTERM（修复于 `fab9cd2`）
- [x] **I3** `OutboxPublisher::run()` bug-shape（doc 标记为 TEST HELPER ONLY）
- [x] **I2** audit-service 仍有死方法 + 孤儿 schema — 后续清理
- [x] **I2-fu1** `scripts/openapi-codegen.js:90` 硬编码修复（commit `0412751`）
- [x] **I5** `MySqlOutboxRepo::new(pool.clone())` per-call — 移到 `AppState` 优化
- [x] **I5-fu1** iam 剩余 7 处 `MySqlOutboxRepo::new` 全部改走 `state.outbox_repo.as_ref()`

---

## Phase 5-A-2 — 放弃 Consul Connect，改用 registrator（2026-07-07）

- [x] 全部 10 个 commit 完成并合并

---

## Phase 5-B — JWT 签名验证 + service-jwt 机制（2026-07-07）

- [x] **5B-fu1** `Principal::from_jwt_payload` 加 `#[deprecated]`（commit `99fc5f9`）
- [x] **5B-fu6** principal extractor JWT 验签失败时 warn 日志（commit `99fc5f9`）

---

## Phase 5 — 服务间安全 & 韧性

- [x] **A — mTLS / 服务网格**：实施 Phase 5-A plan（5 个 Task，commits `f5887b1` … `d8da3bf`）
- [x] **B — 韧性三件套**：ResilientClient + gateway 熔断 + 删除未使用韧性脚手架
- [x] **B-fu1** 韧性三件套扩面到 token→user 等服务间直调

---

## Phase 6-A — Trace 链闭环 + 事件总线生产化

- [x] **T1 — Envelope.trace_id producer 自动注入**（覆盖 I4）
- [x] **T2 — OTel 全覆盖 + tracing_mw 提取共用**
- [x] **T3 — Consumer transient/permanent 分级 + bounded redelivery**（覆盖 I1）
- [x] **T4 — Outbox 24h 归档**（覆盖 I6）
- [x] **T4-fu1** `docs/USER_MANUAL.md` outbox section 加 archive 30d 一行

### Phase 6-A 后续项

- [x] **T3-fu1** `Basic.Return` 静默丢消息修复
- [x] **T3-fu2** `republish_confirm_nack_does_not_ack_original` 集成测
- [x] **T2-fu1** `span_from_envelope_trace_id` 搬到 `libs/common-event/src/span.rs`
- [x] **T2-fu3** outbox-publisher 拆为独立 crate
- [x] **T2-fu2** 删 docs 中 `tracing_mw` 旧说明
- [x] **T3-fu3** testcontainers `sleep()` → poll-with-deadline
- [x] **T3-fu4** api-gateway perm_consumer 行为变更发布说明

---

## Phase 6-B — Config-Service 自举

- [x] **T1** `libs/common-config` 加 `ConfigServiceProvider`
- [x] **T2** `Loader` 集成 provider + 热更新 watch
- [x] **T3** token-service 第一个接入

---

## Phase 8 — 多租户基础

### Phase 8.1 — 骨架

- [x] V003 迁移：`iam_tenants` 表 + default seed + `iam_users.tenant_id`
- [x] `libs/jwt-ctx::Principal.tenant_id` + tid claim 解析
- [x] `libs/jwt-ctx::TenantScope` extractor
- [x] `libs/common-db::WithTenantScope` helper
- [x] `services/iam-service::token_svc::issue_access_token` 加 tenant_id 参数 + JWT `tid` claim
- [x] iam-service 5 处 Envelope::new callsite 注入 tenant_id
- [x] `contracts/events.yaml` 加 `iam.tenant.created` + codegen
- [x] `POST /iam/tenants` 注册接口 + 同事务建 admin + 发事件
- [x] `LoginRequest.tenant_slug` + login tenant-scoped lookup
- [x] testcontainers 集成测 + 全量 docker 单测回归 + 手工验收

### Phase 8.2 — 业务表 tenant_id 迁移

- [x] brainstorm 8.2 spec：表清单、迁移顺序、索引策略
- [x] 迁移 V004（按服务）：每个服务各自 ALTER 业务表加 tenant_id
- [x] sqlx 查询点全量改造
- [x] roles 改为 tenant 内
- [x] envelope.tenant_id consumer 侧注入
- [x] Task 7: 回归 + 手工验收

### Phase 8.3 — 全面贯通 + 跨租户检测

- [x] gateway 中间件：对比 `Principal.tenant_id` 与请求中可能携带的 tenant_id 一致性
- [x] `Principal::from_jwt_payload` 缺 `tid` → 401
- [x] `scripts/check-tenant-scope.js`：lint 报警
- [x] platform-admin 引入 `cross_tenant: bool` 路径
- [x] e2e 跨租户访问黑名单
- [x] `UserService::get_claims` 加 tenant_id 过滤

### Phase 8.3-fu1 — iam-service 点查询 TenantScope 迁移

- [x] `libs/common-db::WithTenantScope` 加 `where_id(id)` helper
- [x] `user_svc.rs` 9 方法签名 + 11 SQL 迁 `scope: &TenantScope`
- [x] `role_svc.rs` 7 方法 + 7 SQL 同上
- [x] `org_svc.rs` 8 方法 + 7 SQL 同上
- [x] 路由 handler 签名改 `TenantScope` extractor
- [x] 集成测试 + e2e YAML

### Phase 8.3-fu2 — ntf / cfg / file 点查询迁移 + check-tenant-scope 启发式升级

- [x] notification-service：8 处 point queries 迁 `scope: &TenantScope`
- [x] config-service：5 处 point queries 同上
- [x] file-service：8 处 point queries 同上
- [x] e2e 黑名单扩展 CT-012~CT-020

---

## Phase 9 — Java 8 集成示例 + Audit SDK

- [x] `sdk/java/audit-service-sdk` 骨架 + 模型 + 异常 + 入口
- [x] `examples/java8-webapi` 父 POM + 目录骨架
- [x] `examples/java8-webapi` 配置内部 Maven 仓库 Reposilite
- [x] `demo-shared` 模块 ApiResponse/ErrorCode/PageReq
- [x] `demo-webapp` 基础（POM + 启动 + 配置 + Bean + 全局异常）
- [x] Task 实体 + TaskStatus 枚举 + TaskRepository
- [x] TaskService 业务主轴（CRUD + 状态机）
- [x] JwtAuthInterceptor + @RequiresPermission AOP
- [x] TaskController + 3 MockMvc tests
- [x] AuthDemoController + 2 tests
- [x] PermissionDemoController + 3 tests
- [x] AuditPublisher + AuditDemoController + 2 tests
- [x] ConfigPoller + ConfigDemoController + 1 test
- [x] NotificationDemoController + 1 test
- [x] FileDemoController + 2 tests
- [x] TaskService + AuditPublisher 接入
- [x] Task read endpoints 强制 TASK_VIEW 权限
- [x] `scripts/demo.sh` + README 跑通教程

---

## 本 session 完成（2026-07-07）

- [x] **5B-fu1** `Principal::from_jwt_payload` 加 `#[deprecated]`（`libs/jwt-ctx/src/principal.rs`）
- [x] **T4-fu1** `docs/USER_MANUAL.md` outbox section 加 archive 30d（`docs/user-manual/02-architecture.md`）
- [x] **5B-fu6** principal extractor JWT 验签失败 warn 日志（`libs/jwt-ctx/src/principal.rs`）
- [x] **5A-2-fu1** `registrator:master` 保持不变（确认无需固化 digest）
- [x] **5A-2-fu2** `docker-example/` 已删除
- [x] **5B-fu2** `api-gateway` 统一到 `common-jwt::JWKSCache`（删 jwt-simple/rsa/base64；commits `0f14e1a` … `d295987`）
- [x] **4.4-fu3** gateway perm_epoch 5s 本地内存短缓存（commit `0f14e1a`）
- [x] gateway 限流从 governor 迁 Redis 滑动窗口（多实例共享；commit `ddf44a0`）
- [x] common-ratelimit ZADD member bug 修复（`format!("{}:{}", identity, now_ms)`；commit `ac28dfa`）
- [x] **OpenAPI registration sweep**：iam-service `org_units::*` 6 handler + `permissions::*` 5 handler + 9 DTO ToSchema/IntoParams + openapi.rs paths/components/tags + openapi.yaml 手工补齐（dump-openapi binary 因 utoipa 递归序列化栈溢出 pre-existing，本地无法运行；commit `a2cd626`）
- [x] **4.3-fu1** announcement delivery 批量 INSERT（`notification-service/src/db.rs` `insert_deliveries_batch` QueryBuilder helper + 2 处调用点改走 + 100-user 回归测试；commit `f5373be`）
- [x] **service-jwt test helper**：`common-jwt::ServiceJwtCache::prime_cache` + 3 个 notification-service 集成测试改 3-arg IamMemberClient + `tests/fixtures/test_private.pem` 测试 RSA 私钥（commit `5481ab6`）
- [x] TODO.md 清理 stale 项：`I1`/`I4`/`I6`/`T2-fu2`/`T3-fu3`/`T3-fu4`/`5A-fu1` 早已完成移入 done.md，标 `[x]` 并加 commit 索引
- [x] 2026-07-07 决策：1.0 内不增加安全相关功能，**Phase 5-C 基础设施 TLS** + **5B-fu4 TLS termination** 归入 TODO.md 新增「Roadmap 2.0」区（R2.1 + R2.2），待 1.0 落地 + 真实合规需求时再 brainstorm
- [x] **5B-fu5** CI 对齐 5B spec：`cargo test --workspace --all-targets` → `--lib`（spec §7.2 早定 --lib，但 CI 漏改）。同步 `docs/guides/testing.md` + `docs/user-manual/06-deployment.md`。本地实测 74 unit tests 全过。附 token-service 编译修复（jwt-simple 0.12.16 trait `RSAKeyPairLike` 未为 `RS256KeyPair` 实现 `set_key_id`，用 inherent `with_key_id` 替）
- [x] **5B-fu3** spec + plan + T1 events：spec (`docs/superpowers/specs/2026-07-08-phase5b-fu3-notification-iam-event-sync-design.md`) + plan (`docs/superpowers/plans/2026-07-08-phase5b-fu3-notification-iam-event-sync.md`, 10 task) + T1 `contracts/events.yaml` 新增 3 事件 `iam.user.role.{assigned,unassigned}` + `iam.user.dept.changed`，codegen 自动生成 `IamUserRoleAssigned/Unassigned/IamUserDeptChanged` struct + routing-key 常量（commits `a284e4e` + `239653d`）。T2-T10 待下 session 跨多 commit 执行。
- [x] **5B-fu3 T2-T10 全量落地**：iam 6 emit 点 + membership_event helper (`1d6c95f`) · GET /iam/internal/membership-bootstrap 端点 + 7 case 集成测 (`3092b29`) · notification V003 `ntf_user_projection` + apply/resolve/replace 5 helper (`d88eb7f`) · `notification.membership-events` durable consumer + 3 typed subscribers + bindings (`cd77a79`) · bootstrap startup wiring + 标 `#[deprecated]` IamMemberClient::{by_role/by_dept/expand_scopes} (`69cc97b`) · publish_announcement 改走本地 SQL + 改 9 测试 (`0740463`) · 8 case membership_projection_test + test-case/5b-fu3-event-sync.md live e2e (`b290ecc`) · docs sync architecture + ai-context (`fabd0c5`)。`cargo test --workspace --lib`: 74 passed。CrossTenant（platform admin）路径暂仍走 deprecated HTTP 端点；删除归 Roadmap 2.0 清理 phase。

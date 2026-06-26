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

#### Phase 4.0 — 数据表命名规范统一（**前置，必须先合并**）

> 现状：所有 Rust 服务共用 MySQL `superpowers` 库，且无任何表名隔离约定，已经出现 iam/audit 两个 `login_logs` 撞名 —— 谁先跑迁移谁占位。继续往里加 outbox 等表会让坑越挖越大。
> Spec: `docs/superpowers/specs/2026-06-26-phase4.0-table-naming-design.md`

- [ ] 新建 `contracts/db-conventions.md`，定前缀映射：`iam_/audit_/token_/ntf_/cfg_/file_/prue_`
- [ ] 排查 dev 环境 `login_logs` 撞名实际情况，确定归属
- [ ] 各服务 `V00X__rename_tables.sql` 一次性 RENAME 存量表
- [ ] sqlx 查询、e2e fixture、测试 setup、文档同步改名
- [ ] `scripts/check-db-naming.js` CI gate
- [ ] `agents.md` / `services/<svc>/AGENTS.md` 更新规范
- [ ] `coder-migrate-db` skill 模板加前缀默认值

#### Phase 4 主任务（4.0 合并后开工）

- [ ] 基础设施：`infrastructure/docker-compose-infra.yml` 加 RabbitMQ + `definitions.json` 预声明 topology
- [ ] `contracts/events.yaml` 启用首批三事件 + envelope schema
- [ ] `scripts/contracts-gen.js` 扩展：events.yaml → Rust event struct + routing-key 常量
- [ ] **`libs/common-event` 完整抽象**：Publisher / Consumer / Envelope / OutboxRepo<DB> trait + lapin RabbitMQ 实现
- [ ] iam-service 加 `iam_outbox_events` 表 + 业务事务里写 outbox
- [ ] iam-service 加 `[[bin]] outbox-publisher`（同镜像切 CMD）
- [ ] audit-service 加 `audit_events` 表 + RabbitMQ consumer + 删除旧 HTTP 写入端点
- [ ] audit-service 补 OTel 接入（验收要求端到端 trace 链）
- [ ] e2e 用例 `audit-event-flow.yaml`
- [ ] 验收：RabbitMQ 拔掉/恢复、consumer panic → DLQ、Jaeger 端到端 trace 链

### Phase 5 — 服务间安全 & 韧性（P1）

- [ ] **mTLS / 服务网格**
  - [ ] 评估 Consul Connect vs Linkerd2（侧重内存占用与运维复杂度）
  - [ ] 服务间通信切换为 mTLS（rustls 已就位）
  - [ ] 淘汰应用层 `require_service_token` 方案

- [ ] **韧性三件套（在 `libs/common-http` 里以 `tower::Layer` 形式集成）**
  - [ ] 全局超时（`TimeoutLayer`，默认 3s 可配）
  - [ ] 幂等方法自动重试（`RetryLayer` + 指数退避 + jitter）
  - [ ] 熔断器（`failsafe-rs` 或自实现 closed / open / half-open 三态）
  - [ ] gateway 对每个下游 service 一个独立 breaker，状态通过 metrics 暴露

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
- [ ] `events.yaml` — 在 Phase 4 落地，否则删除避免误导
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

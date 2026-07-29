# pre-v1-todo.md — 1.0 上线前架构评审整改清单

> 生成时间：2026-07-29
> 生成方式：架构师视角对全仓库做完整扫描（服务地图、CI/CD、契约层、事件总线、多租户、观测栈、文档信任链、密钥治理）后按优先级排出。
> 每一项都需要先 spec → plan → 实施，遵循 `.trae/rules/project_rules.md` §1.3。
> 已完成项归档到 `done.md`，勿手动编辑本文。

---

## 结论（三句话版）

项目在**微服务纪律**（8-item 规范、契约层、事件总线、多租户、trace 全链路、outbox）已做到中大型团队 rarely-see 的高度；但**基础设施基座、部署路径、文档信任链、密钥治理**正在拖后腿——现状是「代码/规范层是 A，运维/发布/文档层是 C-」。要出 1.0 生产版本，下面的 P0/P1 必须处理，否则规范纪律建立的信誉会被一次事故一夜掏空。

---

## P0 级问题（阻塞 1.0 上线）

### P0-1 · 文档信任链断裂

**证据**：

- `AGENTS.md`（根） line 4 明确要求先读 `docs/ai-context.md`；`.trae/rules/project_rules.md` §0 同样。
- **`docs/ai-context.md` 不存在**。真身在 `docs-site/ai-context.md`。
- `docs/decisions/`、`docs/guides/` **整个目录不存在**，但被 README、user-manual、`.trae/rules`、多份 spec、TODO.md 反复引用。
- `docs/user-manual/11-reference.md` 直接给出 7 个死链（`docs/decisions/ADR-*.md`、`docs/guides/canary-rollout.md` …）。
- `docs/superpowers/retros/2026-07-retrospective.md` 已经自述过一次「ADR 目录缺失」，但没落实。

**影响**：AI 协作入口失效、新人 onboarding 死路、review 时凭 spec 找不到 ADR。这条不修，投入在 `.trae/rules/` 的规范纪律等于对未来接手的自己说谎。

**修法**（1 天）：

1. 决策 canonical 路径：**要么** `docs/` **要么** `docs-site/`，**二选一**。推荐留 `docs-site/`（onboarding + user-manual 已就位、有 nginx 容器）。
2. 全仓库 grep 替换所有 `docs/ai-context.md` `docs/decisions/` `docs/guides/` `docs/runbooks/` → `docs-site/…`。
3. 加一个 CI gate：`scripts/check-doc-links.js` —— 扫 `.md` 里的相对 `docs/` `docs-site/` 链接，`stat` 不存在即失败。
4. `AGENTS.md`/`project_rules.md` 首屏统一改成 `docs-site/ai-context.md`。

---

### P0-2 · CI/CD 只覆盖 2/9 服务，deploy 靠 `git pull`

**证据**（`.gitlab-ci.yml`）：

- 只 build `api-gateway` + `token-service` 两个 image。其余 7 个服务（iam / notification / audit / config / file / prue_backend / iam-outbox-publisher）**没有 CI 镜像构建 job**。
- `deploy` stage 是 `rm -rf ~/coder-micro-services && cp -r . …; docker-compose down; pull; up -d` —— **一次全停机**，无健康门禁、无回滚、无灰度。
- `rust_test` 只跑 `--workspace --lib`，Phase 4/6-A 强调的 **testcontainers 集成测试**不在 CI（README 反复说要跑 `run-tests-docker.js`）。
- **没有** `cargo clippy` / `cargo fmt --check` / `cargo audit` / `cargo deny` / `eslint` 门禁。
- **没有** 依赖漏洞扫描（RustSec Advisory / npm audit / OWASP）。
- **没有** SAST（trivy image / grype / semgrep）。

**修法**（1 周）：

1. 补齐 7 个服务的 `build_<svc>` job，用 matrix + `changes:` rule 只 build 动过的服务。
2. `test` stage 加 `cargo clippy --workspace -- -D warnings` + `cargo fmt --check`；本地 `.pre-commit-config.yaml` 联动。
3. `cargo audit` + `cargo deny check` 每周 scheduled pipeline。
4. `deploy` 从 stop-the-world 换成**滚动更新**：`docker-compose up -d --no-deps --scale=2 <svc>` + `curl /healthz` 门禁 + `docker-compose up -d --scale=1`；或直接推 Phase 7 降级决策的反悔（见 P1-4）。
5. Rust image build 加 `--cache-from` + registry-based layer cache（当前一次全量 build 应该是 15-30 min）。

---

### P0-3 · 明文密码入库（Compliance blocker）

**证据**（`.gitlab-ci.yml`）：

```yaml
DEPLOY_PASSWORD: Ewq321#@!    # 明文在 git 历史里
DEPLOY_USER: kudi              # 生产 SSH 用户
```

`README.md` 里也重复了这两条（`DEPLOY_PASSWORD: Server admin password`）。

**修法**（半天）：

1. **立即**改 GitLab CI/CD Variables 里的 **masked + protected**，从 yaml 里删。
2. 换掉该服务器上的对应密码 & SSH 私钥（假设**已泄露**，因为 git 历史 = 泄露）。
3. `docs-site/runbooks/secret-rotation.md` 补一节「已泄露密钥的应急替换」。
4. 装个 pre-commit `gitleaks` / `trufflehog` 挡再次重犯。

---

### P0-4 · 基础设施单点，无 HA 与备份

**项目启动时现状**：

- MySQL / Redis / Consul / RabbitMQ 各**单容器 1 副本**（`infrastructure/docker-compose.yml`）。
- 无 backup runbook（`docs-site/runbooks/` 只有 secret-rotation.md）。
- Phase 7（K8s / K3s 化）已被降级到 **P10**（README 里也说「维持 docker-compose 作为唯一部署方案」）。
- 服务级也无 replica 概念，`docker-compose scale` 靠谱与否没验证过。

**已完成的阶段（2026-07-29）**：

- ✅ **备份 Runbook**：`docs-site/runbooks/backup.md`，覆盖 MySQL / Redis / RabbitMQ / Consul 的手动备份与恢复全流程，含一键备份脚本 skeleton

**剩余**（手动备份已覆盖，HA 部分归 Roadmap 2.0）：

- MySQL：每日 `mysqldump` + 异地拷贝 runbook（哪怕手动 cron）。
- Redis：`SAVE`+`BGSAVE` runbook + 数据丢失影响面书面化（perm_epoch 丢了就变成「所有 token 立即失效」）。
- RabbitMQ：`definitions.json` 已预声明 topology，加 `rabbitmqctl export_definitions` runbook。
- **1.0 之内**：MySQL 主从（1 主 1 从，读写分离先不做，只做灾备）；Redis 换 Sentinel 或至少 AOF+RDB 双开；Consul 至少 3 节点。
- **1.0 之后**：Phase 7 反悔 —— K3s 3 节点 + Consul 3 副本 + RabbitMQ mirrored queue。

---

## P1 级问题（1.0 结束前收）

### P1-1 · 观测栈只搭了骨架，没搭闭环

- ✅ Prometheus + Grafana + Jaeger + OTel collector 都跑起来
- ❌ **无 Alertmanager**，Prometheus 抓的指标没有告警
- ❌ **无 SLO/SLI 文档**（错误预算、可用性目标全靠感觉）
- ❌ **无集中式日志**（onboarding 里已标 TODO：ELK/Loki/SLS,实际未选型）
- ❌ Grafana 只有 `Gateway Overview`；audit/config/file/notification/prue/iam 无专属 dashboard
- ⚠️ README 里 audit/config/file OTel 三格显示 `—`，但 TODO.md `Phase 6-A T2` 说已经全接了 —— **README 漂移**

**修法**：

1. 加 Alertmanager + 5 条基础规则（服务 down、5xx 率 >1%、P99 >500ms、outbox 堆积 >1000、DB 连接池饱和）。
2. 写 `docs-site/slo.md`：每个服务的可用性目标（例如 gateway 99.9%、iam 99.5%）+ 错误预算。
3. 选型日志方案 —— **Loki 最省资源**，跟 Grafana 无缝。补一个 promtail 采集容器 stdout。
4. 补 6 个服务的 Grafana dashboard（`services/<svc>/observability/dashboard.json`，跟服务代码一起管理）。
5. README 表格 OTel 三格 `—` → `✅`（做的活对不上文档 = 未来的迷雾）。

---

### P1-2 · 数据库物理隔离缺失

现状：全部 8 个服务共享 `superpowers` 单库。已用**表名前缀**隔离（`iam_/audit_/token_/…`），且有 CI gate（`check-db-naming.js`）。

**问题**：

- 逻辑隔离 ≠ 物理隔离。任何一个服务写坏了共享库，全平台挂。
- Migration 冲突：8 个服务的 `V00X__*.sql` 各自 forward-only，同库跑 —— 迁移执行顺序在 dev/prod 是否严格一致没做规约。
- 多租户 tenant_id 加了列，但**行级安全**（RLS）没上；靠应用层 `TenantScope` extractor + `check-tenant-scope.js` lint 兜底 —— lint 已知有 delegated 豁免（Phase 8.3-fu2 提到）。

**修法**（评估先行，1 周）：

1. 出一份 **ADR-005 · Shared-DB vs Per-Service-DB**：明确列成本、收益、切换代价。当前路径其实等于「共享 DB / 契约层隔离」，是刻意选择还是路径依赖？
2. 短期加**每服务独立 DB user + 权限**：`iam_svc` 用户只能 `iam_*` 表，即使代码写错也打不穿隔离。这是**零迁移成本**的安全网。
3. `check-tenant-scope.js` 的 delegated 豁免统计一下当前有多少，跟踪治理率。

---

### P1-3 · 网关 = 自研 + SPOF

`api-gateway` 是自写的 Axum 服务（路由、限流、验签、trace 注入、canary、perm_epoch 全塞在里面）。TODO 里有一条早就挂着：

> - [ ] 评估自研 router 替换为 Envoy / Pingora 的成本/收益

**为什么这条现在是 P1**：

- 网关是**每一个请求都过的 SPOF**，出了 bug 全平台挂
- 现在往里塞了限流 + JWT 验签 + service-jwt 分发 + trace 注入 + Consul 服务发现 + canary + perm_epoch 检查 —— **单个 crate 已经背负 6+ 责任**
- 自研 = 自己修所有 CVE、性能 bug、并发 bug（`common-http::ResilientClient` 也在这条路径上）
- Envoy / Pingora / Traefik 有专业团队和熟成度

**修法**（评估先行，1 周）：

- 出 **ADR-006 · Gateway 长期策略**：给出 3 个选项（继续自研 / Envoy sidecar / Pingora）的 6-9 个月路线图。不需要立刻切，但**要有决策**。
- **短期加固**：给 gateway 加多副本 + Nginx L4 前置（当前 nginx 已有 `infrastructure/nginx/nginx.conf`，评估把它扶正到生产入口）。

---

### P1-4 · Phase 7（K8s 化）被降级到 P10 的决策要重审

TODO 里的原话：

> **2026-07-19 决策**：当前无客户使用 K8s，Phase 7 从 P2 降级至 P10。

这个决策**只考虑了外部客户**，但内部**已经在生产上单点跑**（P0-4）。K8s 或 K3s 不是给客户看的功能，是**给自己活下去用的**。

**建议**：

- 如果 1.0 就是**离线私有化交付**给客户装（`release/dist-template/`），那 docker-compose 是合理选择。**但同时补上 P0-4 里的 HA 手段**。
- 如果内部有 SaaS/公网 tenant，K3s 至少要在**内部环境**跑起来 —— 现在的 docker-compose 单机撑不住任何流量峰值。

---

### P1-5 · 测试深度不够

现有的：

- ✅ 单元测试（`cargo test --lib`）
- ✅ testcontainers 集成测试（run-tests-docker.js）
- ✅ E2E（tests/e2e，smoke / api / ui）
- ✅ 契约检查 gate（openapi / permissions / error-codes）

**缺的**：

- ❌ **Contract test**（Pact / consumer-driven）：SDK ↔ Service 之间没有独立契约验证；OpenAPI 只在服务端 dump，SDK 侧只有 wiremock 假 mock。
- ❌ **Load test / soak test**：没有基线数据（当前系统能扛多少 RPS？多少并发 tenant？outbox publisher 每秒吞吐？）
- ❌ **Chaos test**：Phase 6-A T3 加了 consumer 分级，但没有跑过 chaos 演练（拔 RabbitMQ、拔 Redis、SIGKILL publisher）
- ❌ **性能回归门禁**：CI 里没有 benchmark 保留

**修法**（渐进）：

1. 1.0 前：至少 1 次**手动**负载摸底（k6 或 locust），出一份基线报告 `docs-site/perf-baseline.md`。
2. 1.0 后：Pact + 1 条 chaos 场景（RabbitMQ + iam publisher）。

---

## P2 级问题（技术债，稳定后处理）

| 问题 | 状态 (2026-07-29) | 证据 | 建议 |
|---|---|---|---|
| **19 个 `libs/common-*` crate 过度拆分** | ✅ 已出约定 | `libs/` 下有 17 crate（不含 common/ facade） | [CONVENTIONS-libs-crate-strategy.md](docs-site/decisions/CONVENTIONS-libs-crate-strategy.md)：拆分原则、依赖图、版本策略、变更指南 |
| **`kudi-http-client` vs `common-http::ResilientClient` 潜在重叠** | ⬜ 推迟到 2.0 | 前者 SDK 底座（用户 JWT），后者服务间调用（韧性三件套） | SDK plan §0 已做语义区分。2.0 后用 trait/type 做编译期隔离 |
| **frontend e2e 覆盖率评估** | ⬜ 待评估 | `frontend_todo.md` 7 服务对接完成 | 跑一次 vitest coverage report，验证关键路径覆盖率 |
| **outbox archive 指标暴露未做** | ⬜ 待实现 | outbox-publisher 无 `/metrics` HTTP 端点 | 在 publisher `main.rs` 加 `TcpListener` on `:9184` 暴露 prometheus metrics |
| **`test-case/` 手工验收步骤** | ⬜ 评估中 | `test-case/00-setup + 41/42/43/…` 25+ 步骤 | 作为 runbook 持续使用；接入 CI 需要挑选最具价值场景 |
| **Java SDK 端点漂移** | ⬜ 待处理 | Java SDK 部分端点与现网不一致 | 写 `check-java-sdk-drift.js` 或废弃过期端点 |
| **前端 i18n namespace 错位** | ⚠️ 422 处错位·有 plan | 422 处 `t('menus.X')` 但 yaml 中 `X` 在 `buttons:` namespace | [plan](docs/superpowers/plans/2026-07-10-frontend-i18n-namespace-fix-plan.md)：363 keys 从 `buttons:` → `menus:` 迁移。非 3 个 key，是系统性 bug |

---

## 值得保留的地方（给平衡感）

- **契约层设计**（`contracts/error-codes.yaml` + `permissions.yaml` + `events.yaml` + `db-conventions.md` + codegen）——这是**大团队才会做**的纪律，现在在这里跑得很干净。
- **事件总线 + outbox + 分级重投 + trace_id 自动注入**（Phase 4 + 6-A）——比 90% 的公司做得更严谨。
- **多租户从骨架到点查询的**分阶段推进（8.1 → 8.2 → 8.3 → 8.3-fu1 → 8.3-fu2）——每一步都有 spec/plan/progress，是**教科书级的演进节奏**。
- **service-jwt + JWKS + 5B 系列**——服务间信任模型很干净。
- **`.trae/rules/` + `AGENTS.md` + skills 目录**——AI 协作纪律搭得比大部分开源项目认真。
- **`test-case/` 编号化手工验收**——虽然没进 CI，但当作 runbook 已经非常有用。

---

## 推进优先级建议

**本周已完成（2026-07-29）**：

- ✅ P0-1（文档链接修复）+ P0-3（密钥泄露修复）+ P0-4（备份 Runbook）
- ✅ P0-2（CI 补齐 9 服务 build + clippy/fmt/eslint + audit/deny main 触发 + 滚动 deploy）
- ✅ P1-1（Alertmanager 告警 + 6 服务 Grafana dashboard + SLO 文档）
- ✅ P1-2（ADR-005 DB 隔离）+ P1-3（ADR-006 网关策略）+ P1-4（K3s 决策重审）
- ✅ P1-5（负载摸底模板 + k6 脚本）

**下一步**：

1. **P2 条目全部** — 按优先级逐个处理（技术债，不阻塞 1.0）
2. **MySQL 主从**（P0-4 剩余） + **首次负载摸底执行**（填充 perf-baseline.md 数据）

**一个月内**：P0-4 MySQL 主从、Pact 契约测试评估。

**三个月内**：2.0 路线 — K3s 内部环境 + Per-Service-DB 拆分 + 网关方案选型

---

## 进度跟踪

- [x] P0-1 文档信任链断裂（2026-07-29 ✅ 57 文件、CI gate check-doc-links.js）
- [x] P0-2 CI/CD 补齐 7 服务 + 静态检查 + 漏洞扫描（2026-07-29 ✅ 9 服务 build + clippy/fmt + eslint + audit/deny main 触发 + 滚动 deploy）
- [x] P0-3 明文密码泄露（2026-07-29 ✅ 从 .gitlab-ci.yml / README 移除，加 gitleaks pre-commit）
- [x] P0-4 基础设施备份 Runbook（2026-07-29 ✅ docs-site/runbooks/backup.md，覆盖 4 组件手动 SOP）
- [x] P1-1 观测栈告警/SLO/日志（2026-07-29 ✅ Alertmanager + 5 告警规则 + 6 服务 Grafana dashboard + SLO 文档）
- [x] P1-2 DB 物理隔离评估（2026-07-29 ✅ ADR-005 Shared-DB vs Per-Service-DB + 1.0 DB user 权限加固方案）
- [x] P1-3 网关长期策略 ADR（2026-07-29 ✅ ADR-006 自研/Envoy/Pingora 三选项 + 2.0 触发条件）
- [x] P1-4 K8s 化决策重审（2026-07-29 ✅ K3s decision review: 1.0 Docker Compose / 2.0 K3s + 触发条件）
- [x] P1-5 测试深度—负载摸底（2026-07-29 ✅ docs-site/perf-baseline.md 模板 + tests/load/baseline.js k6 脚本；Pact/Chaos 推迟到 1.0 后）
- [ ] P2 全部条目（2026-07-29 已评估：P2-1 ✅ 有约定文档、P2-2~P2-7 ⬜ 不阻塞 1.0，见上方表格状态列）

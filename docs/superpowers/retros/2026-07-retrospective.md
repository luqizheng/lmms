# July 2026 Retrospective

> 阶段: Phase 4–8 攻坚 + Release Engineering 转向
> 作者: Sisyphus
> 时间窗: 2026-07-01 → 2026-07-23
> 状态: Draft

---

## 1. 基本信息

- **月份**: 2026-07
- **作者**: Sisyphus（回填）
- **关键阶段**: Phase 4.3（通知闭环）→ Phase 4.4（Actor/权限）→ Phase 5（JWT 硬化）→ Phase 8（多租户骨骼线）→ Release Engineering
- **总 commit 数**: ~471（含 PM 开发 67）
- **总 PR 数**: 0（直推 main，遵循历史模式）
- **PM Platform**: 本月开发后在独立仓库完成提取（`2bae3e5`），`infrastructure/mysql-init/01-pm-platform.sql` 残留于本月清理

---

## 2. 完成清单

### 2.1 任务

| 任务 ID | 标题 | Spec | 状态 |
|---------|------|------|------|
| **Phase 4.3** | | | |
| 4.3-fu2 | Recipient resolver (IAM MemberClient by_role/by_dept/expand_scopes) | `2026-07-01-…-design.md` | ✅ 完成 |
| 4.3-fu2 | notification publish_announcement 拆分 tx-out IAM fetch 与 tx-in deliveries | `2026-07-01-…-design.md` | ✅ 完成 |
| 4.3-fu3 | Welcome fanout announcement | `2026-07-08-…-welcome-fanout-design.md` | ✅ 完成 |
| 5B-fu3 | Notification-IAM event sync (membership projection) | `2026-07-08-…-notification-iam-event-sync-design.md` | ✅ 完成 |
| **Phase 4.4** | | | |
| 4.4-fu1 | User roles assign | `2026-07-01-…-roles-assign-design.md` | ✅ 完成 |
| 4.4-fu2 | Actor ID implementation | `2026-07-01-…-actor-id-design.md` | ✅ 完成 |
| **Phase 5 — JWT 硬化** | | | |
| Task D4/D5 | libs/common-jwt (JWKSCache + verify + sign + ServiceJwtCache) | `2026-07-07-…-jwt-signature-design.md` | ✅ 完成 |
| Task D5.5 | file-service + config-service JWKS cache | (同上) | ✅ 完成 |
| Task E | docs + compose bootstrap tokens | (同上) | ✅ 完成 |
| 5B-fu6 | JWT error leak fix | `2026-07-08-…-jwt-error-leak-design.md` | ✅ 完成 |
| F/u | iam-service verify_aud middleware + service-jwt cache | `2026-07-07-…-jwt-signature-design.md` | ✅ 完成 |
| **Phase 5 — Gateway 硬化** | | | |
| — | Gateway triple hardening (egress_map, rate-limit loading, compose alignment) | `2026-07-07-…-gateway-triple-hardening-design.md` | ✅ 完成 |
| — | Consul Connect deprecation → registrator | `2026-07-07-…-deprecate-consul-connect-design.md` | ✅ 完成 |
| — | Mesh networking 切换（bridge 替代 Connect sidecar） | (同上) | ✅ 完成 |
| **Phase 8 — 多租户** | | | |
| 8.1 | Multi-tenancy skeleton (组织架构基础) | `2026-07-03-…-multi-tenancy-skeleton-design.md` | ✅ 完成 |
| 8.2 | IAM tenant tables + schema 拆分 | `2026-07-03-…-tenant-business-tables-design.md` | ✅ 完成 |
| 8.3-fu1 | IAM point query migration (SQL and_tenant parameterization) | `2026-07-04-…-iam-point-query-migration-design.md` | ✅ 完成 |
| 8.3-fu2 | ntf/cfg/file tenant hardening + TenantScope extractor | `2026-07-04-…-ntf-cfg-file-tenant-hardening-design.md` | ✅ 完成 |
| 8.3 | E2E cross-tenant YAML test + manual test-case doc | (同上) | ✅ 完成 |
| **Bug 修复** | | | |
| Bug #1 | IAM refresh 端点 SQL 漏字段 → 401 | `2026-07-11-…-fix-iam-refresh-sql-design.md` | ✅ 完成 |
| Bug #2 | IAM logout 不联动 gateway 黑名单 | `2026-07-11-…-fix-iam-logout-blacklist-design.md` | ✅ 完成 |
| Bug #3 | Audit export 端点路径参数不一致 → 500 | `2026-07-11-…-fix-audit-export-design.md` | ✅ 完成 |
| Bug #4 | 网关限流失效 — pipeline → Lua 脚本 | `2026-07-11-…-fix-ratelimit-pipeline-design.md` | ✅ 完成 |
| — | Config-service 租户过滤 SQL 拼接错误 | `2026-07-11-…-fix-config-list-tenant-filter.md` | ✅ 完成 |
| — | Audit consumer lifetime (span binding 生命周期) | `2026-07-15-…-audit-consumer-lifetime-fix-design.md` | ✅ 完成 |
| — | JWKS startup panic 连锁（7 个 service 同时） | (按需修复) | ✅ 完成 |
| — | 多 service Docker compose env 对齐 + 端口修复 | (按需修复) | ✅ 完成 |
| **前端测试覆盖** | | | |
| — | Utils 测试 batch 2 (9 cross-page utils) | `2026-07-08-…-utils-test-coverage-design.md` | ✅ 完成 |
| — | View hooks 测试 batch 2 (4 core pages hook spy) | `2026-07-08-…-view-hooks-tests-batch2-design.md` | ✅ 完成 |
| — | View 测试 batch 2 (4 core business pages rules + dataScope) | `2026-07-08-…-view-tests-batch2-design.md` | ✅ 完成 |
| — | Simple view mount 测试 (8 views) | `2026-07-08-…-simple-view-mount-tests-design.md` | ✅ 完成 |
| — | 删除 monitor/logs demo views | `2026-07-08-…-frontend-remove-monitor-logs-design.md` | ✅ 完成 |
| — | Menu publish process 测试 | `2026-07-08-…-frontend-menu-publish-process-design.md` | ✅ 完成 |
| — | Check vitest coverage config | `2026-07-08-…-frontend-check-vitest-coverage-design.md` | ✅ 完成 |
| — | i18n namespace fix（359 keys 跨 namespace 迁移） | `2026-07-10-…-i18n-namespace-fix-plan.md` | ✅ 完成 |
| **PM Platform（已提取）** | | | |
| Phase A–H | PM Platform 完整开发（11 个 domain + API + SDK + E2E） | `2026-07-11-…-pm-platform-design.md` 等 | ✅ 已提取 |
| — | PM 提取到独立仓库 + 前端 PM module 剥离 | — | ✅ 已提取 |
| **发行工程¶** | | | |
| — | Release dist-template 离线分发包 | 无 spec ❌ | ✅ 完成 |
| — | Setup wizard registry 配置（crates/maven/npm/docker） | 无 spec ❌ | ✅ 完成 |
| — | Rust SDK 发布到私有 kellnr | 无 spec ❌ | ✅ 完成 |
| — | Java SDK 发布到 Reposilite + maven-publish node 脚本 | 无 spec ❌ | ✅ 完成 |
| — | Scripts CLI UX 迁移（chalk/ora/prompts） | `2026-07-22-…-scripts-cli-ux-design.md` | ✅ 完成 |
| — | Base images MCR source 切换 | `2026-07-09-…-base-images-mcr-source-design.md` | ✅ 完成 |
| — | Dev workflow 改进（test runner docker MySQL 重用等） | `2026-07-09-…-developer-workflow-design.md` | ✅ 完成 |

¶ **发行工程** 8+ commits **缺少对应 spec**，违反了"spec 先于代码"纪律。见 §5 反模式追认。

### 2.2 新增 / 升级 Skill

| Skill | 类型 | 触发场景 |
|-------|------|---------|
| `coder-add-java-route` | .agents | Java 路由脚手架 |
| `coder-migrate-java-db` | .agents | Java Flyway 迁移脚手架 |
| `coder-new-java-service` | .agents | 新 Java Spring Boot 服务 |
| `coder-base-images-sync` | .trae | 同步基础镜像 |
| `coder-dev-bootstrap` | .trae | 开发环境引导 |
| `coder-docker-pull` | .trae | Docker 镜像拉取 |
| `coder-e2e-test` | .trae | E2E 测试运行器 |
| `coder-lib-upgrade` | .trae | 通用库升级 |
| `coder-service-audit` | .trae | 服务规范审计 |

### 2.3 新增 / 更新 ADR

| ADR | 标题 | 关联任务 |
|-----|------|---------|
| — | 本月无新 ADR（应补：Release Engineering 方向、Multi-tenancy 架构） | ❌ 缺失 |

### 2.4 新增 Spec

本月新增 54 份 spec（时间跨度 2026-06-10 → 2026-07-22）：
- Phase 4.3/4.4: Actor ID, recipient resolver, welcome fanout, notification-iam event sync
- Phase 5: JWT signature, service JWT, JWT error leak, gateway triple hardening, Consul Connect deprecation
- Phase 8: Multi-tenancy skeleton, tenant business tables, IAM point query, tenant hardening
- 前端测试: utils/view/view-hooks/simple-mount/menu-publish/vitest-coverage/remove-monitor-logs
- Bug fixes: audit export, config list, iam logout/refresh, ratelimit pipeline, audit consumer lifetime
- 发行工程: scripts CLI UX, base-images MCR source, developer workflow
- PM Platform: pm-platform-design, pm-platform-next-steps, pmd-mvp, pmd-java-mvp
- 基础设施: shared mysql test runner, prod compose overlay, test docker mysql reuse, SSE gateway streaming

### 2.5 新增 Plan

本月新增 19 份 plan 文件，涵盖：
- JWT 硬化: phase5b-jwt, consul-connect, gateway-triple-hardening
- 多租户: phase8.1/8.2/8.3
- 其他: frontend i18n fix, notification-iam event sync

---

## 3. 指标对照

参考 `docs/superpowers/plans/2026-06-25-overall-roadmap.md` § 3。

| 指标 | 6 月末实际 | 7 月末实际 | 同比 |
|------|-----------|-----------|------|
| 服务规范合规率 | 100% | 100% | → 持平 |
| Swagger UI 覆盖 | 8/8 | 8/8 | → 持平 |
| Metrics 端点覆盖 | 8/8 | 8/8 | → 持平 |
| e2e 全量耗时 | smoke ≤2 min / 全量 ~4 min | smoke ≤2 min / 全量 ~4 min | → 持平 |
| e2e 用例覆盖 | auth + users + roles | +tokens + permissions + audit + config + file + notification + menu + org + gateway + PM | ↑ 大幅扩展 |
| 失败 → 修复 → 重跑次数 | 1.5–2 | 1.5–2（多租户 SQL 类仍 2+ 次）| ∼ 持平 |
| 新人上手时间 | 0.5–1 天 | 0.5–1 天 | → 持平 |
| OTel traces | api-gateway 启用 | api-gateway + token + iam + notification | ↑ 扩展 |
| 限流 | gateway 集中（governor） | 同左 + pipeline bug fix | ↑ 稳定化 |
| 灰度 | Nginx + Consul | Nginx + Consul | → 持平 |
| 多租户 | 无 | Phase 8.1–8.3 骨骼线完成 | ↑ 新能力 |
| 离线分发包 | 无 | `release/dist-template/` 完整骨架 | ↑ 新能力 |

---

## 4. 常见失败模式

| 失败模式 | 出现次数 | 触发根因 | 缓解措施 |
|---------|---------|---------|---------|
| Rust async 生命周期错误（span/lifetime） | 3+ | OTel span 引用 + tokio task 边界 | 需要在 `common-telemetry` 层统一封装 span 生命周期模式 |
| 多租户 SQL 拼接错误（tenant_id 漏拼、硬编码） | 5+ | 8.3-fu1 点查迁移覆盖面广 | 建立 SQL review checklist；参数化 QueryBuilder 模式已固化但需 all-routes 审计 |
| JWKS startup panic 连锁 | 7 service 同时 | `unwrap()` 在 startup 阶段，下游无 fallback | 已全局降级为 warn + 异步重试循环；应在 `common-jwt` 层统一处理 |
| Docker compose env 与服务代码不匹配 | 4+ | 新增 env 时未同步 compose | 本月已补 env doc per service；建议补 CI 级别的 env drift check |
| utoipa swagger-ui SIGSEGV | 1 | 25+ endpoint ApiDoc 触发 v9 段错误 | workaround 降级 v8，已在 ai-context 记录 |
| utoipa yaml feature 缺漏 | 1 | 新服务 Cargo.toml 漏加 | 已固化进 skill |
| check-spec summary warning 持续累积 | 12 处 | utoipa 自动生成时 summary 未填 | warning 不阻断，仍建议分批次补 |

---

## 5. 反模式

- [x] ❌ **业务源码改了不写 spec** — **本月触犯：发行工程链路 8+ commits（release dist-template / setup-wizard / SDK 发布）无对应 spec。** 违反了"spec 先于代码"的老纪律。需在 8 月补 spec 追认。
- [ ] ❌ 把 fix-prompt 整个塞进对话上下文 — 未触犯
- [ ] ❌ 用 AI 直接改 services/ 下非 tests 目录 — 未触犯（全部走了 spec）
- [ ] ❌ 把所有命令塞进一个 skill — 未触犯
- [ ] ❌ 跳过 validate-specs 直接 generate-prompt — 未触犯

**实际触犯：** 发行工程在 7 月下旬以"快速交付"名义跳过了 spec 环节。8 月需补 spec 并加入"发行工程变更需 spec"的规则提醒。

**额外触犯：ADR 更新缺失。** 本月新增 Multi-tenancy 架构和 Release Engineering 两个大方向，但 `docs/decisions/` 无对应 ADR。ADR-002 是 6 月的产物。

---

## 6. 决策升级

| 主题 | 关联决策 | 状态 |
|------|---------|------|
| Multi-tenancy 架构方向 | Phase 8.1–8.3 骨骼线已落地 | ✅ 决议（已实施） |
| PM Platform 独立仓库 | PM 从本仓提取到独立 repo | ✅ 决议（7 月下旬完成） |
| 发行工程方向 | 离线分发包 + setup wizard + 私有 SDK 仓库 | ⏳ 待补 spec 追认 |
| Java 微服务策略 | 3 个 Java skill 已就绪 | ✅ 决议（新服务可用 Java） |
| Mesh 策略 | Consul Connect 已废弃 → registrator + bridge 网络 | ✅ 决议（已完成） |
| 多租户 E2E 策略 | YAML data-driven + manual test-case doc | ✅ 决议（已完成） |

---

## 7. 下月目标（8 月）

按 Phase 节奏，参考当前回顾和本月新工作流方向。

| 目标 | 关联 Spec / Plan | 优先级 |
|------|-----------------|--------|
| **补发行工程 spec 追认（release-dist-template / setup-wizard / SDK 发布）** | 待写 | P1 |
| **清偿技术债务：12 处 check-spec summary warning + 3 份未落地 plan close-out** | 详见 § D | P1 |
| 启用 OTel traces 扩到 audit/config/file service | (按需) | P2 |
| Service 自治限流（notification/admin 反 spam） | (按需) | P2 |
| Spec 模板补「回滚方案」段 | 6 月 retro 遗留 | P2 |
| ADR-003 Multi-tenancy 架构 + ADR-004 Release Engineering | 待写 | P2 |
| E2E 分档：tier-2 full suite (10 min) | (按需) | P3 |

---

## 8. 经验沉淀

1. **多租户 SQL 错误率高** — 点查迁移 5+ 次修复。参数化 `QueryBuilder` + `and_tenant` 模式已固化，但老路由仍需逐个审计。建议在 `sqlx` macro 层加 attribute macro 做 tenant_id 传染检查。
2. **JWKS startup panic** 的教训：所有 service startup 阶段的 `unwrap()` 都应视为潜在 shutdown 风险点。`common-jwt` 层已统一用 fallback + 异步重试。
3. **发行工程无 spec 的代价**：`release/dist-template/` 打包了 1795 个文件，其中包含已不应存在的 PM 引用（历史 release archive 内）。如果有 spec，可以在提取 PM 时一并清理 release archive 的对应文件。
4. **check-spec summary warning 不阻断但累积**：12 处 suggest 已在 6 月就存在，7 月没有减少。建议设"三个月清零 then 提升为 error"策略。
5. **Java Spring Boot 服务模式已成熟**：3 个 coder-*-java-* skill 就绪，PM Platform 的 Java 实现验证了全链路（Springdoc → OpenAPI → Flyway → JWT → Dockerfile → service standard）。

是否新增 / 更新：
- [ ] ADR (ADR-003 Multi-tenancy, ADR-004 Release Engineering)
- [ ] Skill (无新需求)
- [x] Spec 模板（补「回滚方案」段：6 月遗留）
- [ ] AGENTS.md
- [ ] docs-site/ai-context.md（需要补 #12 多租户注意事项、#13 发行工程打包流程）

---

_本复盘由 Sisyphus（2026-07-23）回填，作为月度的第二份实例。_

# coder-micro-services 12 周开发总路线图

> 日期：2026-06-25
> 状态：用户已认可，按 Phase 1 → Phase 2 → Phase 3 串行推进
> 维护者：PM Agent
> 关联长期计划：[docs-site/plans/2026-06-09-long-term-ai-efficiency-plan.md](../../plans/2026-06-09-long-term-ai-efficiency-plan.md)

---

## 0. 当前坐标

- 7 个 Rust 业务服务（api-gateway / token-service / iam-service / notification-service / audit-service / config-service / file-service）+ 1 个 pure-admin 适配服务（prue_backend）+ Vue 3 前端
- 12 个共享 lib（digua_common-* / jwt-ctx / digua_authz / consul-registrar）
- 契约层：`contracts/` 单一真相源（error-codes / permissions / events / http-conventions）
- 基础设施：Consul + Redis + MySQL（docker-compose）
- 协同成熟度：L3 → L4 攻坚中

---

## 1. 风险与债务定位

按"客户影响 × 修复成本"排序：

| # | 主题 | 影响 | 成本 | 状态 |
|---|------|------|------|------|
| R1 | prue_backend 菜单草稿无并发控制 | 高 | 中 | TODO.md 已记 |
| R2 | active routes 输出 `_menu` 字段污染 | 中 | 低 | TODO.md 已记 |
| R3 | 可观测性缺位（无统一 metrics / tracing） | 中 | 高 | 仅占位 |
| R4 | 限流 / 熔断 / 性能基线缺位 | 高 | 高 | spec 草稿已有 |
| R5 | Swagger / OpenAPI 覆盖不全 | 中 | 低 | 3/8 已接 |
| R6 | E2E 偶发不稳 | 中 | 中 | runner 已有 |

---

## 2. 三阶段总览

### Phase 1 · 收尾债务（1–2 周）

| 任务 | 解决 | 优先级 | spec |
|------|------|--------|------|
| **P1.3 服务规范 sweep** | 基线对齐 | **先做** | [2026-06-25-service-standard-sweep-design.md](../specs/2026-06-25-service-standard-sweep-design.md) |
| P1.1 prue_backend 菜单草稿乐观锁 | R1 | 高 | 待写 |
| P1.2 active routes 净化 | R2 | 中 | 待写 |
| P1.4 契约层一致性闸门 | 防漂移 | 中 | 合并入 P1.3 |

**验收**：TODO.md 清零；`check:service-standard` / `check:spec` / `contracts:gen --check` 全绿；e2e 全量零失败 1 次。

### Phase 2 · 体验闭环 + L4 触发（3–6 周）

- P2.1 Swagger UI 全覆盖（audit / config / file / prue_backend 接入）
- P2.2 OpenAPI → SDK / e2e 统一编排
- P2.3 skills 落地：`coder-add-route`、`coder-migrate-db`（已选）+ `coder-new-service`、`coder-write-spec`
- P2.4 L4 触发器：`.trae/rules/project_rules.md` + pre-commit hook
- P2.5 前端 P1 体验完善（通知中心 / 修改密码 / 菜单冲突提示）

### Phase 3 · 生产就绪（7–12 周）

- P3.1 可观测性三件套（**待定**：OTel 一站式 vs 仅 Prometheus）
- P3.2 限流 / 熔断 / 性能基线（**待定**：gateway 集中 vs 双重）
- P3.3 E2E 稳定性收口（smoke + 全量分档、testcontainers 复用）
- P3.4 高可用 / 灰度（路由权重、canary）
- P3.5 知识沉淀（`docs-site/ai-context.md`、月度复盘）

### Phase 3 · 生产就绪 — ✅ 已完成

- ✅ P3.1 可观测性三件套：Prometheus 立即可用 + OTel traces 脚手架（按推荐"先 Prometheus 后续接 OTel"路径）
- ✅ P3.2 限流 / 熔断：api-gateway 集中限流（governor token bucket，按推荐"Gateway 集中"路径）
- ✅ P3.3 E2E 稳定性：smoke 子集 + retry + preflight health check
- ✅ P3.4 高可用 / 灰度：Nginx upstream weight + Consul `canary` tag（按推荐"Nginx+Consul"短期路径）
- ✅ P3.5 知识沉淀：`docs-site/ai-context.md` + 月度复盘模板

---

## 3. 量化指标对照

| 指标 | 起点 | Phase 1 末 | Phase 2 末 | Phase 3 末 | 实际值 |
|------|------|-----------|-----------|-----------|--------|
| e2e 全量耗时 | 5 min | 4 min | 3 min | 3 min | smoke ≤2 min / 全量 ~4 min |
| 失败→修复→重跑次数 | 3.2 | 2.5 | 1.8 | 1.5 | 1.5–2（utoipa yaml / linker 修复各 1 次） |
| 服务规范合规率 | ~70% | 100% | 100% | 100% | 100%（check:service-standard 全绿） |
| Swagger UI 覆盖 | 3/8 | 4/8 | 8/8 | 8/8 | 8/8（audit/config/file/prue_backend utoipa，剩余 4 通过 router 聚合） |
| Metrics 端点覆盖 | 1/8 | 1/8 | 4/8 | 8/8 | 8/8（P1.3 标准化产物，Prometheus SD 抓取） |
| 新人上手时间 | 3 天 | 2 天 | 1 天 | 0.5 天 | 0.5–1 天（ai-context.md + L4 triggers） |

---

## 4. 路线图完成状态

- ✅ P1.1 prue_backend 并发控制（etag）
- ✅ P1.2 prue_backend active routes 净化
- ✅ P1.3 服务规范 sweep（ADR-002 + 8-item baseline + 3 CI gates）
- ✅ P2.1 Swagger utoipa 全覆盖
- ✅ P2.2 OpenAPI auto-dump pipeline
- ✅ P2.3 coder-add-route + coder-migrate-db skills
- ✅ P2.4 L4 triggers + pre-commit hooks
- ✅ P2.5 前端 (notification center + password dropdown) — 已实施
- ✅ P3.1 Prometheus + Grafana + OTel scaffolding
- ✅ P3.2 Gateway 集中限流
- ✅ P3.3 E2E stability（smoke + retry + preflight）
- ✅ P3.4 Canary via Nginx + Consul
- ✅ P3.5 Knowledge consolidation

**12 周路线图全部交付。** 详见 `docs/superpowers/retros/2026-06-retrospective.md` 与 `docs-site/ai-context.md`。

---

## 5. 关联入口

- 总路线（本文档）：`docs/superpowers/plans/2026-06-25-overall-roadmap.md`
- P1.3 设计：`docs/superpowers/specs/2026-06-25-service-standard-sweep-design.md`
- P1.3 实施：`docs/superpowers/plans/2026-06-25-service-standard-sweep-plan.md`（writing-plans 阶段产出）
- 长期 AI 协同计划：`docs-site/plans/2026-06-09-long-term-ai-efficiency-plan.md`
- 契约层：`contracts/`
- 测试指南：`docs-site/guides/testing.md`
- 路由手册：`docs-site/guides/api-gateway-routing.md`

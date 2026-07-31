# 8 月路线图提案

> 日期：2026-07-23
> 状态：Proposal（待用户确认）
> 前置：7 月 Retrospective（`docs/superpowers/retros/2026-07-retrospective.md`）

---

## 0. 当前坐标

- **Roadmap 状态**：12 周原始路线图（P1-P3）✅ 全部交付
- **7 月产出**：Phase 4.3→8 多租户攻坚 + JWT 硬化闭环 + 发行工程转向
- **PM Platform**：已提取到独立仓库
- **Service 规范合规率**：100%（持平）
- **未清偿债务**：79 处 check-spec summary warning / 0 ADR 更新 / 发行工程缺 spec

---

## 1. 风险排序

| # | 主题 | 严重度 | 说明 |
|---|------|--------|------|
| R1 | 发行工程无 spec 追认 | 中 | 8+ commits 无设计文档，ADR 空白 |
| R2 | 79 处 summary warning 持续累积 | 低 | 不阻断但损伤 OpenAPI 消费体验 |
| R3 | ADR 矩阵空白（多租户 + 发行工程） | 低 | 新协作者无架构参考 |
| R4 | OTel 只覆盖 4/8 服务 | 中 | audit/config/file 无 trace，排障盲区 |
| R5 | 弹性欠账：service 自治限流未做 | 低 | gateway 集中限流已有 |

---

## 2. 候选方向

### P1 · 文档基建（1 周）

把 7 月欠的文档债还清。

| 任务 | 关联 | 预计 |
|------|------|------|
| **ADR-003**: Multi-tenancy 架构决策 | Phase 8.1–8.3 | 1h |
| **ADR-004**: Release Engineering 架构 | 发行工程 | 1h |
| **发行工程 spec**: release/dist-template 设计 | 离线分发包 | 2h |
| **发行工程 spec**: setup-wizard 设计 | 交互式引导 | 2h |
| **发行工程 spec**: SDK 发布策略 | kellnr + Reposilite | 1h |
| **Spec 模板**: 补「回滚方案」段 | 6 月 retro 遗留 | 0.5h |

**验收**：`docs-site/decisions/` 新增 2 份 ADR；`docs/superpowers/specs/` 新增 3 份 spec；spec 模板更新。

### P2 · 技术债务批量清偿（1 周）

用 sub-agents 并行一次性补完 79 处 summary。

| 任务 | 关联 | 预计 |
|------|------|------|
| **audit-service**: 补 11 处 summary | check-spec | 并行 |
| **config-service**: 补 16 处 summary | check-spec | 并行 |
| **file-service**: 补 7 处 summary | check-spec | 并行 |
| **iam-service**: 补 30+ 处 summary | check-spec | 并行 |
| **notification-service**: 补 8 处 summary | check-spec | 并行 |
| **OpenAPI 漂移 CI 验证** | 确认无回归 | 1 pass |

**验收**：`check-spec` 零 warning；`scripts/dump-openapi-specs.sh && git diff --exit-code` 全绿。

### P3 · 可观测性扩展（1-2 周）

| 任务 | 关联 | 预计 |
|------|------|------|
| audit-service OTel 启用 | 6 月 retro 目标 | 1 天 |
| config-service OTel 启用 | 同上 | 1 天 |
| file-service OTel 启用 | 同上 | 1 天 |
| 跨服务 trace e2e 验证 | gateway → token → iam | 1 天 |

### P4 · 弹性增强（持续）

| 任务 | 关联 | 预计 |
|------|------|------|
| Service 自治限流（notification spam） | 6 月 retro 目标 | 2 天 |
| E2E tier-2 全量套件分档 | 同上 | 2 天 |
| ResilientClient 接入（digua_common-http） | phase4-perf-circuit-limit | 3 天 |

---

## 3. 推荐执行顺序

```
Week 1 (7/23–7/31) → P1 文档基建
Week 2 (8/1–8/7)  → P2 技术债务清偿
Week 3 (8/8–8/14) → P3 可观测性扩展
Week 4+            → P4 弹性增强（按需启动）
```

**理由**：P1 先把 7 月的欠账补了然后关掉；P2 清零 warning，消除 openapi 消费侧的摩擦；P3 补排障盲区；P4 看时间窗口决定是否启动。

---

## 4. 不做的方向（明确）

| 放弃项 | 理由 |
|--------|------|
| **编码新业务功能** | 当前无需求驱动 |
| **Service Mesh 升级** | service 数 < 15，没有必要 |
| **性能压测基线** | 已有 prometheus 监控 + smoke e2e，P0 不紧迫 |
| **PM 相关任何工作** | 已提取到独立仓库 |

---

## 5. 关联入口

- 7 月 Retro：`docs/superpowers/retros/2026-07-retrospective.md`
- Roadmap 总览：`docs/superpowers/plans/2026-06-25-overall-roadmap.md`
- Spec 目录：`docs/superpowers/specs/`
- ADR 目录：`docs-site/decisions/`
- CI 门禁：`scripts/check-spec.js` / `scripts/check-service-standard.js`

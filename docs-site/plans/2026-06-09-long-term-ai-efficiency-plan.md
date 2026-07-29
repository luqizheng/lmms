# coder-micro-services 长期 AI 协同提效计划

> 日期：2026-06-09
> 状态：执行中
> 目标：让 AI 真正成为「一体化协作者」，不只是一问一答的工具

---

## 0. 当前坐标

- 工程：4 服务微服务（Rust × 2 + Go × 2）+ Consul/Redis/MySQL/Nginx
- 三件核心事：api-gateway 路由重构 / 多语言 SDK / 测试编排 AI 闭环
- 已有资产：6 个 Trae skills（.trae/skills/）、test-orchestrator.ps1、ADR-001、3 份设计+实施 plan

---

## 1. 协同成熟度 5 级模型（人机合一）

| 级别 | 名称 | 描述 | 当前 |
|------|------|------|------|
| L1 | 命令式 | AI 回答单点问题 | ✅ |
| L2 | 流程式 | AI 跑固定脚本/工作流 | ✅ skills 落地 |
| L3 | 协同式 | AI 改代码 / 跑测试 / 渲染报告 | ✅ fix-prompt 闭环 |
| L4 | 自治式 | AI 主动发现任务、跑通闭环 | 目标 |
| L5 | 同源式 | AI 与开发者共享 mental model，自我进化 | 远景 |

**当前 L3 → L4 攻坚中**。

---

## 2. 12 周路线图

### Phase 1：固化 L3 闭环（第 1-2 周）

**目标**：现有 skills 全部跑通，无人工补丁。

- [ ] 6 个 skills 在 3 个真实任务中验证
- [ ] `test-orchestrator.ps1 loop` 全程跑通 1 次零失败
- [ ] fix-prompt 模板被实际使用 3 次以上
- [ ] 在 AGENTS.md 头部加「Skill 索引」段

**验收**：跑 `./scripts/test-orchestrator.ps1 loop` 一键到底。

---

### Phase 2：扩展 skills 覆盖（第 3-4 周）

**目标**：补齐缺失工作流的 skill。

- [ ] `coder-new-service` —— 脚手架新服务的 checklist
- [ ] `coder-add-route` —— 交互式生成 `*.router.toml`
- [ ] `coder-write-spec` —— 把口语需求 → YAML 规格
- [ ] `coder-migrate-db` —— GORM AutoMigrate 触发与验证
- [ ] `coder-ci-check` —— 本地跑 `.gitlab-ci.yml` 等价命令

**验收**：新增一个 service 路由，整个流程不离开 IDE。

---

### Phase 3：L4 自治触发（第 5-8 周）

**目标**：AI 主动识别任务并启动对应 skill。

- [ ] 写 `.trae/rules/project_rules.md`：
  - 改动 `services/*/src/*.rs` → 自动建议 `coder-run-tests`
  - 改动 `routes/*.router.toml` → 自动建议 `coder-route-validate`
  - 改动 `sdk/**/pom.xml` → 自动建议 `coder-sdk-publish`
  - 新建 `*.yaml` 在 `tests/specs/` → 自动建议 `coder-spec-to-code`
- [ ] 加 `pre-commit` hook 触发 `validate-specs` + `cargo fmt` + `go vet`
- [ ] 加 `post-failure` hook 自动渲染 fix-prompt
- [ ] GitLab CI 加 `ai-readiness` 任务：检查文档/spec 是否齐全

**验收**：用户改代码后，AI 主动说「是否跑一下测试？」

---

### Phase 4：知识沉淀 + 进化（第 9-12 周）

**目标**：让项目自身的知识反哺 AI。

- [ ] 写 `docs-site/ai-context.md` —— 一份给 AI 看的项目速览
  - 服务清单 + 端口 + 依赖
  - ADR 摘要（特别是 ADR-001）
  - 已知陷阱清单（cargo ring LNK2019、Consul 健康检查、PowerShell dot-source）
- [ ] 把 `test-orchestrator.ps1` 的输出训练成固定 schema，让 AI 学会解析
- [ ] 写 `scripts/lib/AI-Observability.ps1`：跑测试时同步记录耗时分布、失败模式
- [ ] 月度 review：把高频修复动作抽象为 skill 或脚本

**验收**：新人（或新 AI 实例）打开仓库 5 分钟内能上手。

---

## 3. 长期习惯清单（每个开发者每天做）

1. **改测试代码前**先 `coder-fix-tests` 渲染提示词
2. **改路由配置后**先 `coder-route-validate` 再 commit
3. **新增 SDK 包后**先 `pnpm run sdk:test` 三遍跨版本
4. **写新需求**先 `coder-write-spec` 沉淀成 YAML，不直接写代码
5. **每次失败修复闭环**在 `reports/` 留痕，季度归档

---

## 4. 评测指标

| 指标 | 起点 | 12 周目标 |
|------|------|----------|
| 跑全量测试总耗时（loop） | 5min | 3min |
| 失败 → 修复 → 重跑平均次数 | 3.2 | 1.5 |
| 单次部署 YAML 改动→上线 | 30min | 8min |
| 新人上手时间（含 AI 协同时） | 3 天 | 0.5 天 |
| skills 触发准确率 | — | ≥ 80% |

---

## 5. 反模式（要避开的）

- ❌ 把 fix-prompt 整个塞进对话上下文（截前 20 行就够）
- ❌ 业务源码改了不写 ADR/spec
- ❌ 用 AI 直接改 `services/` 下非 tests 目录
- ❌ 把所有命令塞进一个 skill（保持单一职责）
- ❌ 跳过 `validate-specs` 直接 `generate-prompt`

---

## 6. 与现有计划的协同

| 现有 | 关系 |
|------|------|
| `2025-07-11-token-server-api-gateway-plan.md` | 已完成，作为基础设施基线 |
| `2026-06-07-user-service-extends-plan.md` | 进行中，受 Phase 1 测试闭环保护 |
| `2026-06-08-api-gateway-router-config-plan.md` | 已落地，触发 `coder-route-validate` |
| `2026-06-08-multi-lang-sdk-plan.md` | 进行中，触发 `coder-sdk-publish` |
| `2026-06-08-test-orchestrator-plan.md` | 已落地，是 Phase 1 核心 |

---

## 7. 触发器

每周一次站会检查：
- 本周触发过哪些 skills？
- 哪个 skill 失败率最高？
- 是否需要新增 skill？

每月一次复盘：
- 指标表是否向好？
- 反模式是否被触犯？
- ADR 是否需要新增？

---

## 8. 关联入口

- Skills 索引：[.trae/skills/](../../.trae/skills/)
- 测试入口：[scripts/test-orchestrator.ps1](../../scripts/test-orchestrator.ps1)
- 路由手册：[docs-site/guides/api-gateway-routing.md](../guides/api-gateway-routing.md)
- ADR 总览：[docs-site/decisions/](../decisions/)

---

**生成日期**：2026-06-09
**下一次 review**：2026-06-23

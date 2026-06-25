# Monthly Retrospective — Template

> 复制本模板到 `docs/superpowers/retros/YYYY-MM-retrospective.md`。
> 每月第一周完成上月复盘。

---

## 1. 基本信息

- **月份**: YYYY-MM
- **作者**:
- **关键阶段**: Phase X（参考 `docs/superpowers/plans/2026-06-25-overall-roadmap.md`）
- **总 commit 数**:
- **总 PR 数**:

---

## 2. 完成清单

### 2.1 任务

| 任务 ID | 标题 | Spec | 状态 |
|---------|------|------|------|
| Px.y | | `YYYY-MM-DD-…-design.md` | ✅ 完成 / ⚠️ 部分 / ❌ 未做 |

### 2.2 新增 / 升级 Skill

| Skill | 类型 | 触发场景 |
|-------|------|---------|
| | .agents / .trae | |

### 2.3 新增 / 更新 ADR

| ADR | 标题 | 关联任务 |
|-----|------|---------|
| | | |

### 2.4 新增 Spec

列出本月新增的 `docs/superpowers/specs/` 文件。

### 2.5 新增 Plan

列出本月新增的 `docs/superpowers/plans/` 文件。

---

## 3. 指标对照

参考 `docs/superpowers/plans/2026-06-25-overall-roadmap.md` § 3。

| 指标 | 起点 | 月初目标 | 月末实际 |
|------|------|---------|---------|
| 服务规范合规率 | | | |
| Swagger UI 覆盖 | | | |
| e2e 全量耗时 | | | |
| 失败 → 修复 → 重跑次数 | | | |
| 新人上手时间 | | | |

---

## 4. 常见失败模式

本月观察到的失败模式（>1 次出现）：

| 失败模式 | 出现次数 | 触发根因 | 缓解措施 |
|---------|---------|---------|---------|
| | | | |

---

## 5. 反模式

本月触犯的反模式（参考 `docs/plans/2026-06-09-long-term-ai-efficiency-plan.md` § 5）：

- [ ] ❌ 把 fix-prompt 整个塞进对话上下文
- [ ] ❌ 业务源码改了不写 ADR/spec
- [ ] ❌ 用 AI 直接改 services/ 下非 tests 目录
- [ ] ❌ 把所有命令塞进一个 skill
- [ ] ❌ 跳过 validate-specs 直接 generate-prompt

实际触犯：**（记录具体案例）**

---

## 6. 决策升级

本月需要团队 / 用户拍板的事项：

| 主题 | 关联决策 | 状态 |
|------|---------|------|
| | | ⏳ 待定 / ✅ 决议 / ❌ 撤回 |

---

## 7. 下月目标

按 Phase 节奏，参考 `docs/superpowers/plans/2026-06-25-overall-roadmap.md`。

| 目标 | 关联 Spec / Plan | 优先级 |
|------|-----------------|--------|
| | | P0 / P1 / P2 |

---

## 8. 经验沉淀

本月值得固化的经验：

1. （一句话 + 一行证据）
2. ...

是否新增 / 更新：
- [ ] ADR
- [ ] Skill
- [ ] Spec 模板
- [ ] AGENTS.md
- [ ] docs/ai-context.md

---

_模板由 P3.5（2026-06-25）建立。_
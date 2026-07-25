# Design Spec — Template

> 复制本模板到 `docs/superpowers/specs/YYYY-MM-DD-<topic>-design.md`。
> Spec 是"改代码前先有 spec"纪律的载体。合并进同一提交周期。

---

# <标题：动词 + 主体 + 意图>

> 日期：YYYY-MM-DD
> 关联：`docs/superpowers/plans/<相关 plan>.md` 或 `.trae/rules/<规则>.md`
> 范围：影响的 `services/<svc>/` / `libs/<lib>/` / `contracts/` 等
> 风险：低 / 中 / 高（一句话说明）

---

## 1. 现状

### 1.1 具体行为 / bug 复现 / 遗漏事实

（用命令 / curl / 代码片段还原当前状态）

### 1.2 根因

（一句话 + 关键代码指路）

### 1.3 生产影响 / 业务后果

（谁被影响、影响多大）

---

## 2. 修复方案 / 设计方案

### 2.1 方案选择

（列 2-3 个候选 + 选中的理由）

### 2.2 改动清单

| 文件 | 改动 | 原因 |
|------|------|------|
| `services/foo/src/bar.rs` | +12 -4 | 修 X 逻辑 |

### 2.3 数据流 / 调用路径 / 时序图

（Mermaid / ASCII / 文字皆可，必要时补关键代码）

---

## 3. 测试计划

- [ ] 单测：`cargo test -p <svc> <test_name>`
- [ ] 集成测：`cargo test -p <svc> --test <int_test>`
- [ ] E2E：`pnpm -C tests/e2e test:<group>` 或新增 yaml case
- [ ] 手动回归：`curl …` / UI 步骤

---

## 4. 回滚方案

> **必填**：所有 spec 都必须回答"如果这个改动上线后炸了怎么办"。
> 不允许写"不需要回滚"——至少写出"如何 revert 到上一个已知好状态"。

### 4.1 回滚触发条件

（什么信号说明需要回滚：错误率、日志、指标、用户投诉等）

### 4.2 回滚步骤

1. 具体命令 / 操作（`git revert <sha>` / `helm rollback` / `docker compose … pull && up -d` 等）
2. 若有 schema 变更：down migration 或前向兼容路径
3. 若有 config 变更：如何还原
4. 若有 outbox / event / 状态污染：如何清理

### 4.3 回滚耗时估计 + 数据损失面

（分钟级？小时级？是否会丢消息 / 重复消费？）

### 4.4 不能回滚的情况

（如果这个改动一旦上线就无法逆转 —— 比如加密算法迁移 —— 明确说出来，并说明前置校验和分阶段灰度策略）

---

## 5. 已知限制 / 后续工作

- （改动引入的技术债、TODO、后续可扩展点）

---

## 6. 关联 ADR / spec

- ADR-XXX（如果本 spec 沉淀了新的架构决策）
- 前置 spec `YYYY-MM-DD-…-design.md`

---

_模板由 2026-07 retro 建立（补『回滚方案』段），作为 spec 撰写基线。_

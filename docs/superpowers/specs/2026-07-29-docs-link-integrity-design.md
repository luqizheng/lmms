# P0-1 · 文档信任链断裂修复 · Design Spec

- **Date**: 2026-07-29
- **Author**: Sisyphus (架构评审 P0-1)
- **Status**: Approved（本 session 立即实施）
- **Related**: `pre-v1-todo.md` §P0-1

---

## 1. 背景

架构评审发现根 `AGENTS.md` line 4 明确要求先读 `docs/ai-context.md`，但**该文件不存在**——真身在 `docs-site/ai-context.md`。同类死链遍布 README、user-manual、`.trae/rules`、TODO.md、多份 spec 与 7 个服务 AGENTS.md。已知在评审报告里 grep 到 237 处引用 `docs/(ai-context|decisions|guides|runbooks|user-manual|USER_MANUAL)`。

**后果**：AI 协作入口失效、新人 onboarding 死路、review 时凭 spec 找不到 ADR。规范纪律建立的信誉被对未来接手的自己撒谎的死链掏空。

---

## 2. 设计决策

### 2.1 Canonical 路径（双根共存）

| 根路径 | 定位 | 内容 | 状态 |
|---|---|---|---|
| `docs-site/` | **稳定文档 canonical**（面向读者：新人、AI、运维） | `ai-context.md` / `guides/` / `runbooks/` / `decisions/` / `user-manual/` / `onboarding/` / `plans/`（长期路线） | 已存在，完整 |
| `docs/superpowers/` | **研发过程文档**（面向作者：spec / plan / retro） | `specs/` / `plans/` / `retros/` | 已存在，60+ 文档，不搬 |

不做整并、不搬 `docs/superpowers/`（`.trae/rules` §1.3 明确要求每次 spec 都写在此处，历史已积累 60+ 份，搬迁成本大于收益且破坏 spec 引用）。

### 2.2 死链目标（本 spec 的替换范围）

**只**处理 4 个**不存在**的路径前缀：

| 死链前缀 | 修复目标 |
|---|---|
| `docs/ai-context.md` | `docs-site/ai-context.md` |
| `docs/decisions/` | `docs-site/decisions/` |
| `docs/guides/` | `docs-site/guides/` |
| `docs/runbooks/` | `docs-site/runbooks/` |
| `docs/USER_MANUAL.md` | `docs-site/USER_MANUAL.md` |

**本 spec 范围新增（2026-07-29 扫描时发现的隐藏问题）**：

- **`docs/user-manual/` 整体删除**：扫描发现 `docs/user-manual/` 12 份 .md 与 `docs-site/user-manual/` 高度重复——10/12 完全相同，2 份差异（`01-quick-start.md` 缺 setup wizard 段、`README.md` 缺 12 章条目），且 `docs-site/user-manual/` 多一份 `12-release-and-distribute.md`。判定：`docs-site/user-manual/` 是较新 canonical 版本，`docs/user-manual/` 是搬家后未清的旧副本。**删除整目录**，外部对 `docs/user-manual/` 的引用改指向 `docs-site/user-manual/`。用户决策（2026-07-29 question 回复）。

**不**在本 spec 范围内的：

- `docs/superpowers/*` 内部对 `docs/decisions/` 的引用 —— **也**需要改，spec 数量多但改的都是相对链，同样只做指向修复
- `docs-site/.vitepress/dist/` 构建产物 —— 排除（下次构建自动重写）
- `release/dist-template/release/coder-micro-services-1.0.0/` 发布快照 —— 排除（一次性冻结产物）
- `.omo/`、`.superpowers/` 历史 ledger —— 排除
- `node_modules/`、`target/` —— 排除

### 2.3 替换扫描范围

grep 白名单：`*.md` / `*.yaml` / `*.yml` / `*.json` / `*.js` / `*.ts` / `*.rs` / `*.toml`
grep 排除目录：`node_modules` / `target` / `.git` / `docs-site/.vitepress/dist` / `release/dist-template/release` / `.omo` / `.superpowers`

### 2.4 CI Gate 设计

新增 `scripts/check-doc-links.js`：

- 输入：Node.js 单文件脚本，无外部依赖（沿 `scripts/check-service-standard.js` 风格）
- 行为：
  1. 遍历仓库所有 `.md` 文件（同上排除规则）
  2. 用正则抽取 markdown 内联链接 `[text](path)` 与引用式链接 `[text]: path`
  3. 只对**相对路径** + 目标是 `.md` 或目录的做 stat 检查
  4. 跳过：`http(s)://` / `mailto:` / `#anchor` 起头 / `../` 跳出仓库根 / 已知外部
  5. 有死链 → 打印文件行号 + exit 1
- CI 接入：`.gitlab-ci.yml` `test` stage 加 `doc_links_check` job（image node:20）
- 本地：`.pre-commit-config.yaml` 加一条 hook

### 2.5 入口文档修复

- 根 `AGENTS.md` line 4：`docs/ai-context.md` → `docs-site/ai-context.md`
- `.trae/rules/project_rules.md` §0：同上
- 7 个 `services/*/AGENTS.md`：全量替换 4 类死链

---

## 3. 实施步骤

1. **扫描**：grep 输出治理清单（当前估计 237 处，去掉排除目录后应该 <120）
2. **批量替换**：按目标文件分批用 `sed` 或 `Edit` 工具做机械式替换，每批 <20 处
3. **新脚本**：`scripts/check-doc-links.js` 落地
4. **CI**：`.gitlab-ci.yml` + `.pre-commit-config.yaml`
5. **验证**：本地跑 `node scripts/check-doc-links.js`，退出 0
6. **单 commit**：`fix(docs): P0-1 修复文档信任链断裂，锁 canonical 路径 + CI gate`

---

## 4. 验收标准

- [ ] `node scripts/check-doc-links.js` 退出 0
- [ ] `grep -rn 'docs/ai-context\|docs/decisions/\|docs/guides/\|docs/runbooks/'` 在源码文件（去除排除目录）中命中 0 处（除本 spec + `pre-v1-todo.md` 里作为**证据引用**保留的原文）
- [ ] `AGENTS.md` / `.trae/rules/project_rules.md` / 7 个服务 AGENTS.md / README.md 首屏引用指向 `docs-site/` 且路径可达
- [ ] `.gitlab-ci.yml` 有 `doc_links_check` job
- [ ] 单 commit，遵循 `.trae/rules/project_rules.md` §1.2

---

## 5. Out of Scope

- `docs-site/plans/` 里对 `docs/decisions/` 的历史引用（本 spec 会顺手修，但不做归档语义改写）
- Rust source `//! link:` doc-comment 死链（本次不扫，工作量大且非阻塞）
- 已交付的 `release/dist-template/release/…` 快照（冻结产物，不动）

---

## 6. 风险

- 部分 `docs/superpowers/specs/*.md` 里的死链是**历史引用**，改动后会污染 git blame。**接受**——修一次比每次翻 spec 走弯路便宜。
- `docs/user-manual/11-reference.md` 里 7 个死链改指 `docs-site/decisions/`、`docs-site/guides/` 后，如果后续 `docs/user-manual/` 被整并到 `docs-site/`，还得再动一遍。**接受**——本次只做指向正确，不做搬家。

# 补全 update-shell skill 的同步流程缺口，并修复 v-perms 门禁的扫描根

> 日期：2026-10-28
> 关联：`docs/superpowers/specs/2026-10-28-sidebar-menu-rhythm-design.md`、
> `docs/superpowers/specs/2026-10-28-prism-spec-versioning-design.md`、
> 下游 `project-manager/docs/superpowers/specs/2026-10-28-upstream-frontend-sync-design.md`
> 范围：`.agents/skills/update-shell/`、`scripts/check-v-perms.js`
> 风险：低（文档 + 一行扫描根；无 schema / config / 数据面变更）

---

## 1. 现状

### 1.1 具体行为 / bug 复现 / 遗漏事实

**（a）skill 的流程有五处"靠人记得"的缺口。** `update-shell` 是 repo-local skill
（AGENTS.md §②），不随 dist 分发，下游仓库没有它——下游实际是人工读上游文件执行。
2026-10-28 在下游 `project-manager` 执行了一次真实同步，暴露：

- **同步范围没写死取上游 HEAD。** skill 只说祖先取"下游拷贝那一刻"的版本。本次祖先判定为
  `31263caf`，而上游 HEAD 已是 `27334760`，中间还落了 `1f1e47b7`（侧栏菜单节奏）——
  恰恰是下游最关心的那个改动。范围取短就整批漏掉，且漏得悄无声息。
- **共同祖先没有可操作的判定法。** 原 skill 写"无法确定则保守处理并标注祖先未知"，
  把一个能算的问题写成了不能算。实际可用的是内容哈希法：取"已同步文件数最多"的提交。
- **没有"下游本地适配点"清单。** 下游把 `check-i18n-hardcode.js` / `check-vitest-coverage.js`
  的基准分支从上游的 `main` 改成 `master`；三方合并时照搬上游版本会直接把门禁改坏。
- **验收清单不完整，且没有排错入口。** 缺 `check-prism-spec-sync`、`check-i18n-hardcode`、
  `check-i18n-namespace`、`check-vitest-coverage`、`check-spec`；反过来
  `pnpm contracts:check` 在下游**不可用**（见 §1.3）。
- **没有"同步 ≠ 生效"的收尾。** 本次同步成果停在 `chore/upstream-frontend-sync` 分支、
  未合入 master，使用方打开页面看到的仍是旧壳。

**（b）`scripts/check-v-perms.js` 在上下游同时是坏的。**

```bash
$ node scripts/check-v-perms.js
Error: ENOENT: no such file or directory, scandir '.../frontend/src/views'
```

PRISM 壳改造把视图从 `frontend/src/views` 迁到 `frontend/src/shell/views`，脚本的扫描根
没跟着迁。该门禁在两边都是**一运行就崩的空跑**，写操作按钮漏挂权限无人发现。

**（c）行尾问题（下游）。** 下游 `.gitattributes` 只覆盖 `frontend/**`，仓库根 markdown 被
本机全局 `core.autocrlf=true` 检出成 CRLF，导致 `src/styles/prism/tokens.test.ts` 按 LF
匹配 `docs/prism/phase1-visual-direction.md` 附录里的 ```` ```css ```` 围栏块失败，
token 契约测试整套变红；`check-prism-spec-sync.js` 与外部草稿区做**字节**比对，同样误报漂移。

### 1.2 根因

- skill 从没在真实下游同步里跑过——它没有下游副本，靠人读文件执行，流程中依赖记忆的部分
  就没有沉淀成文字。
- 壳改造迁了视图目录，没迁依赖该目录的门禁脚本。
- 下游 `.gitattributes` 的覆盖范围是按"前端壳"划的，没跟着新引入的仓库根规范文档走。

### 1.3 生产影响 / 业务后果

- 下游同步后**看似完成实则未生效**，或漏掉上游关键改动；这类失败要到使用方打开页面才发现。
- v-perms 门禁在上下游双双空跑：写操作按钮漏挂权限码无人拦。
- 下游 Windows 环境下 token 契约门禁不可用，token 层漂移无人发现。
- `pnpm contracts:check` 在下游**必然失败**（`contracts/codegen/gen-rust.js` 要写上游布局的
  `libs/common-error/src/codes.rs`，下游无此目录）。若把它列为下游门禁，只会制造"红但不修"的噪声。

---

## 2. 修复方案 / 设计方案

### 2.1 方案选择

| 候选 | 内容 | 结论 |
|---|---|---|
| A | 只补 skill 文档 | ✗ 不足：(b) 的门禁本身是坏的，文档说"跑检查"也跑不起来 |
| B | 补 skill 文档 + 修 v-perms 扫描根，**上下游统一到同一形态** | ✓ **选中** |
| C | 把 `update-shell` 加进 `DOWNSTREAM_SKILL_WHITELIST` 随 dist 分发 | ⏸ 暂不做，见 §5 |

选 B 的理由：把"流程缺口"和"工具缺口"分开修——流程缺口沉淀进 skill 文字，工具缺口直接修代码；
扫描根在**两边**统一到 `frontend/src/shell/views`，下次 cms-sync（`scripts/**` 走 three-way-merge）
不会把下游的本地修复冲掉，也不需要再维护一份下游分叉。

### 2.2 改动清单

| 文件 | 改动 | 原因 |
|------|------|------|
| `.agents/skills/update-shell/SKILL.md` | +82 -8 | 5 步扩 7 步，补齐 §1.1(a) 的五处缺口 + 排错表 + "合入并确认生效" |
| `scripts/check-v-perms.js` | +4 -2 | 扫描根 `frontend/src/views` → `frontend/src/shell/views` |
| `docs/superpowers/specs/2026-10-28-update-shell-skill-gap-fixes-design.md` | 新增 | 本 spec |

skill 改后的流程骨架：

```
第 1 步  确认两端（上游壳真源 / 下游已 scaffold）
第 2 步  算差异（壳基线四目录 + 业务视图排除规则）
第 3 步  定共同祖先（内容哈希法）+ 同步范围 = 祖先..上游 HEAD
第 4 步  产出变更集（A 直接应用 / B 三方合并 / B' 下游适配点 / C 人工裁决）
第 5 步  同步后必做（7 条门禁 + 排错表：行尾、ENOENT、契约格式）
第 6 步  合入目标分支并确认生效  ← 新增，"同步 ≠ 生效"的闭环
第 7 步  报告
```

### 2.3 数据流 / 调用路径

同步本身是文件级 diff，无运行时数据流。要点只有一条**时序**：

```
上游落地新提交 ──► 定共同祖先 ──► 范围取 祖先..上游HEAD ──► A/B/B'/C 分组
                                                                      │
     使用方看到新壳 ◄── 合入目标分支 ◄── 门禁全过 ◄── 逐字比对产物 ◄──┘
```

漏掉最后一环（合入），前序所有工作对使用方都不可见——这正是本次的实际教训。

---

## 3. 测试计划

- [x] `node scripts/check-v-perms.js`（上游）：扫 17 个 `.vue`，✅ all write buttons under v-perms / hasPerms
- [x] 下游同脚本：扫 74 个 `.vue`，✅ 通过（下游修复在 `project-manager` 提交 `967a433`）
- [x] `node scripts/contracts-gen.js --check`：✅ Contracts in sync
- [x] skill 的 7 个步骤与 2026-10-28 实际执行过程逐条比对，无遗漏项
- [x] 手动回归：下游合并后 dev server 提供的 `SidebarMenuItem.vue` 含上游新增的 `is-group` 标记

---

## 4. 回滚方案

### 4.1 回滚触发条件

- `check-v-perms.js` 在扫描到真实写操作按钮后误报率飙升，导致门禁变成噪声；
- skill 改动被下游误读，执行了错误的同步范围。

### 4.2 回滚步骤

1. `git revert <本提交 sha>`（或 `git checkout <上一个已知好 sha> -- .agents/skills/update-shell/SKILL.md scripts/check-v-perms.js`）
2. 无 schema / config 变更，无需 migration 或配置还原。
3. 无 outbox / event / 数据写入，无数据面污染。

### 4.3 回滚耗时估计 + 数据损失面

分钟级；数据损失面为零（纯文档 + 一个只读扫描脚本）。

### 4.4 不能回滚的情况

无。本改动不涉及不可逆变更。

---

## 5. 已知限制 / 后续工作

- **`update-shell` 仍是 repo-local**，下游仓库没有副本，实际靠人工读上游文件执行。
  是否加入 `release.js::DOWNSTREAM_SKILL_WHITELIST` 随 dist 分发，待定——分发会让下游拿到
  的是**上游视角**的 skill，而其中 B' 小节描述的是下游自己的适配点，语义上更适合留在上游。
- 下游 `check-i18n-hardcode.js` 的 `SCAN_DIRS[0]` 仍指向不存在的 `frontend/src/views`
  （上下游相同），导致其中"相对基准分支的新增文件"子检查在两边都空跑。本轮未处理——
  它不影响硬编码扫描本身（扫描遍历 `SCAN_DIRS`，含真实存在的 `frontend/src/shell`）。
- 下游 CRLF 问题的根因在下游 `.gitattributes`，已在下游提交 `18c3bed` 修掉；上游用
  `* text=auto eol=lf` 全覆盖，不受影响。

---

## 6. 关联 ADR / spec

- `2026-10-28-sidebar-menu-rhythm-design.md` —— 本次同步范围里最关键的那笔上游改动
- `2026-10-28-prism-spec-versioning-design.md` —— `docs/prism/` 规范副本与 token 契约的焊点
- 下游 `project-manager/docs/superpowers/specs/2026-10-28-upstream-frontend-sync-design.md`
  —— 本次同步的设计稿（A/B/C 分组与共同祖先判定的原始记录）

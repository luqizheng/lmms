---
name: update-shell
description: |
  把上游 PRISM 壳的更新同步到下游业务系统（diff + 人工裁决定制区）。
  Use when the user says "升级壳/同步壳/update shell/拉取壳的最新改动"。
  取代旧的 cms-sync 前端同步职责（对象从整份肥壳变为薄壳 diff）。
  本 skill 只存在于上游仓库（repo-local，不随 dist 分发）：下游直接读
  low-memory-micro-services/.agents/skills/update-shell/SKILL.md 手动执行。
---

# update-shell Skill

上游 `low-memory-micro-services/frontend/` 是壳的真源。下游业务系统持有壳的一份拷贝，
本 skill 负责把上游改动以**可评审的变更集**形式同步下去。

## 为什么不用 cms-sync

cms-sync 是文件级 managed / three-way-merge / skip 三类策略。旧壳（pure-admin）有几千个
文件、一半是演示代码，同步它等于把上游的每次改动都灌进下游。现在的壳是薄的，
但**下游一定会定制**（品牌色、业务页面、自己的组件），所以仍需要人工裁决。

因此本 skill 做的是：**只同步"壳基线"，把"下游定制"标出来给人看**。

## 第 1 步：确认两端

| 端 | 位置 | 判定 |
|---|---|---|
| 上游 | `low-memory-micro-services/frontend/` | 壳真源 |
| 下游 | 当前仓库的 `frontend/` | 有 `src/styles/prism/` 与 `src/shell/` 即为已 scaffold 的下游 |

若当前仓库既没有 `src/styles/prism/` 也没有 `src/shell/`，那它还没用新壳 ——
先走 `scaffold` skill。

## 第 2 步：算差异

壳基线 = 下列目录的并集：

```
frontend/src/styles/prism/
frontend/src/components/ui/
frontend/src/lib/
frontend/src/shell/          ← 排除下游自己的业务视图
```

业务视图的排除规则：只把上游的 `login/` `dashboard/` `profile/` `NotFound.vue`
`ComingSoon.vue` 视为基线；其余 `src/shell/views/**` 视为下游定制区。

对基线部分逐文件比较（上游有、下游没有 → 新增；两边都有 → 比内容）。

## 第 3 步：定共同祖先

共同祖先 = 上游历史里"下游拷贝那一刻"的提交。**不要凭印象挑**，用内容哈希法：

把上游每个候选提交的基线文件内容与下游逐字比对，取**已同步文件数最多**的提交。

```bash
# 对每个候选提交统计与下游逐字一致的文件数，取最大者
git -C <上游> show <候选提交>:frontend/src/shell/components/SidebarMenuItem.vue | diff -q - <下游>/frontend/src/shell/components/SidebarMenuItem.vue
```

共同祖先确定后，**同步范围 = 祖先..上游 HEAD**：

```bash
git -C <上游> diff <祖先>..HEAD -- frontend
```

⚠️ **范围必须取到上游 HEAD，不能取"祖先当时的 HEAD"**。上游会在同步期间继续落地提交
（例：`1f1e47b7` 侧栏菜单节奏，落在祖先判定之后），范围取短了就整批漏掉。判定完祖先，
先扫一眼 `git -C <上游> log --oneline <祖先>..HEAD -- frontend`，确认范围里含上游最近
落地的那些提交。

## 第 4 步：产出变更集

输出三组，**分清"可直接应用"和"要人裁决"**：

### A. 直接应用（下游没动过）

上游有、下游无，或两边内容完全一致 → 直接同步，无冲突。

### B. 三方合并（下游改过、上游也改过，且不重叠）

用 `git merge-file` 做三方合并：

```bash
git merge-file -p <下游当前版本> <共同祖先版本> <上游新版本>
```

### B'. 下游本地适配点（**不可被上游覆盖**）

下游把部分门禁脚本改成了本地形态，三方合并时按上游版本覆盖会直接把门禁改坏：

| 文件 | 上游形态 | 下游适配 |
|---|---|---|
| `scripts/check-i18n-hardcode.js` | 基准分支 `main` | 基准分支 `master`（下游默认分支） |
| `scripts/check-vitest-coverage.js` | 基准分支 `main` | 基准分支 `master`（下游默认分支） |
| `scripts/check-v-perms.js` | 扫 `frontend/src/shell/views` | 与上游一致 |

合并这些文件时**只取上游的逻辑变化，保留下游的路径/分支适配**。
`frontend/src/styles/prism/tokens.brand.css`（品牌覆盖）同理，是下游定制区。

> `check-v-perms.js` 曾在上游与下游**同时**指着壳改造前就已删除的 `frontend/src/views`，
> 一运行就 ENOENT —— 两边的这个门禁都是坏的。2026-10-28 已在两边都改指
> `frontend/src/shell/views`，上下游就此一致。

### C. 人工裁决（下游改过、上游也改了同一处）

逐处列出：

```
文件：frontend/src/shell/components/Sidebar.vue
冲突点：侧栏当前项的底色
  下游：background: var(--accent-subtle)   ← 下游定制
  上游：background: var(--accent-subtle); color: var(--accent-deep)
建议：取上游（下游的底色本就来自 PRISM 语言，改的是上游补的强调档文字色）
```

**不要自己替人决定**。把清单交给使用者，逐条确认后再应用。

## 第 5 步：同步后必做

```bash
cd frontend
pnpm test:unit                      # 或 pnpm vitest run
npx eslint --max-warnings 0 src build
npx vue-tsc --noEmit --skipLibCheck
pnpm build
cd ..
node scripts/check-prism-tokens.js
node scripts/check-prism-spec-sync.js
node scripts/check-i18n-hardcode.js
node scripts/check-i18n-namespace.js
node scripts/check-vitest-coverage.js
node scripts/check-v-perms.js
node scripts/check-spec.js
```

下游**跑不了** `pnpm contracts:check`（`contracts/codegen/gen-rust.js` 要写上游布局的
`libs/common-error/src/codes.rs`，下游无此目录，直接 ENOENT 退出）——别把它列为门禁。

特别关注：

- **token 契约测试**（`src/styles/prism/tokens.test.ts`）：上游新增/修改 token 后，
  下游的 `tokens.css` 若与之不一致会红 —— 这正是我们要的提示：下游该同步 token 层。
- 下游的 `tokens.brand.css`（品牌覆盖）**不要被上游覆盖**，它是下游定制区。

### 排错入口

| 症状 | 多半是 | 怎么查 |
|---|---|---|
| token 契约测试报"规范附录里没有找到 css 围栏块" | **行尾**：下游 `.gitattributes` 只覆盖 `frontend/**`，仓库根 markdown 被全局 `core.autocrlf=true` 检出成 CRLF，附录里的 ```css 围栏按 LF 匹配就断了 | `file docs/prism/*.md` 看是否 CRLF；给 `.gitattributes` 补 `*.md` / `docs/**` / `contracts/**` 的 `text=auto eol=lf` |
| `check-prism-spec-sync` 报漂移 | 同上（它与外部草稿区做**字节**比对，CRLF 即漂移）；或草稿区 `D:\Workspace\web-concept\design` 才是最新 | 先排除行尾，再考虑 `--sync` 回灌 |
| `check-v-perms` 之类脚本 ENOENT | 脚本扫描根还指着壳改造前的 `frontend/src/views` | 改指 `frontend/src/shell/views` |
| 契约产物 diff 全是格式噪音 | `contracts/codegen/gen-ts.js` 的产出没经过 prettier | `npx prettier --write frontend/src/contracts/*.ts` 后再提交 |

**契约产物再生成后必须过 prettier**，否则 `error-codes.ts` 会被写成单行 union，
提交时是一百多行纯格式 diff。

## 第 6 步：合入并确认生效

同步成果落在分支上**不等于生效**——使用中的人打开页面看到的仍是旧壳。
落一个同步分支、跑完门禁后必须合入目标分支，并确认运行中的产物确实换新。

```bash
git checkout <目标分支>
git merge --no-ff <同步分支>          # 先用 git merge-tree 预演是否无冲突
```

合入后确认：

- 合并后的工作区里，基线文件内容与上游 HEAD 一致（挑 1~2 个关键文件逐字比对）；
- 若有 dev server，确认它提供的就是新代码（查一下关键文件里上游新增的标记是否在）；
- 报告里写明：同步分支名、合入目标分支、以及**合并前下游看不到哪些上游改动**。

## 第 7 步：报告

```
同步完成：
  A 直接应用：12 个文件
  B 三方合并：3 个文件
  C 需裁决：2 处（见下）
  跳过（下游定制区）：src/shell/views/business/**（12 个文件）

待裁决：
  1. ...
  2. ...
```

## 与其他 skill 的关系

- 首次生成用 `scaffold`。
- 壳内部的视觉约束见 `prism-design`。
- 同步后新增的业务页用 `gen-page` 生成。
- 本 skill 是 repo-local（AGENTS.md §②），不随 dist 分发到下游；
  下游直接读上游这份文件手动执行。

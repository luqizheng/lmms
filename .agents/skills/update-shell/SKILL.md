---
name: update-shell
description: |
  把上游 PRISM 壳的更新同步到下游业务系统（diff + 人工裁决定制区）。
  Use when the user says "升级壳/同步壳/update shell/拉取壳的最新改动"。
  取代旧的 cms-sync 前端同步职责（对象从整份肥壳变为薄壳 diff）。
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

## 第 3 步：产出变更集

输出三组，**分清"可直接应用"和"要人裁决"**：

### A. 直接应用（下游没动过）

上游有、下游无，或两边内容完全一致 → 直接同步，无冲突。

### B. 三方合并（下游改过、上游也改过，且不重叠）

用 `git merge-file` 做三方合并：

```bash
git merge-file -p <下游当前版本> <共同祖先版本> <上游新版本>
```

共同祖先取自上游 git 历史里"下游拷贝那一刻"的版本。若无法确定共同祖先，
按"以下游为准 + 上游新增行"的保守策略处理，并在报告里标注**祖先未知**。

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

## 第 4 步：同步后必做

```bash
cd frontend
pnpm vitest run
pnpm exec eslint --max-warnings 0 src
npx vue-tsc --noEmit --skipLibCheck
pnpm build
cd .. && node scripts/check-prism-tokens.js
```

特别关注：

- **token 契约测试**（`src/styles/prism/tokens.test.ts`）：上游新增/修改 token 后，
  下游的 `tokens.css` 若与之不一致会红 —— 这正是我们要的提示：下游该同步 token 层。
- 下游的 `tokens.brand.css`（品牌覆盖）**不要被上游覆盖**，它是下游定制区。

## 第 5 步：报告

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

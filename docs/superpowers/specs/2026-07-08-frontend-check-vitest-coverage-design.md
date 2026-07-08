# Frontend View Test Coverage Linter (F2)

## Status: 🟡 待 user review

`frontend/AGENTS.md`「测试规范」段强制「任一新增 `src/views/**` view，**至少**交付一条 happy-path 单测」—— 但 `frontend_todo.md §4.2` 显示该规范 0 CI 兜底（146 views / 2 tests = 1.4% 覆盖）。本文锁定第一批 utils 单测补齐（commit `60389ad`）之后，**为该规范补 linter** 的设计、规则、接入方式与验收。

---

## 1. 目标

让「新增 view 必须带单测」从口头规范变成 CI gate。脚本只对**新增** view 报错（避免一次性报 144 条历史债）。

---

## 2. 范围

### 2.1 在范围

- 新增 `scripts/check-vitest-coverage.js`：扫 `git diff <base>...HEAD --diff-filter=A` 中的 `frontend/src/views/**/*.vue`，验证同目录或子目录有 `*.test.{ts,tsx}`。
- `frontend/package.json` 加 `check:vitest-coverage` script 调上面。
- 本地 dry-run 验证（临时建 sentinel view → 报错 / 加 test → 通过）。

### 2.2 不在范围（本文 commit）

- ❌ 不接 `pnpm run check` 总串（frontend 无 `check` script，根目录 `check` 是 workspaces check）
- ❌ 不接 pre-commit hook（不强制；用户未来按需）
- ❌ 不接 CI 必跑（用户未来按需在 `.gitlab-ci.yml` 接入）
- ❌ 不扫已存在的 view（避免 144 条历史债一次性爆炸；按"护航未来变更"思路）
- ❌ 不扫 view 重命名（`--diff-filter=A` 只看新增；重命名 = 删除 + 新增，新建那条会走检查）

---

## 3. 设计

### 3.1 base ref 来源（按优先级降序）

1. 命令行参数：`node scripts/check-vitest-coverage.js <base>`（如 `main` / `HEAD~1` / commit SHA）
2. 环境变量 `VITEST_COVERAGE_BASE`
3. 默认 `main`

`git diff` 用三段式 `<base>...HEAD`（仅看 base 之后到 HEAD 的变更），无 commit 时 fallback `git diff --name-only --diff-filter=A <base>`。

### 3.2 扫的路径

```
git diff --name-only --diff-filter=A <base>...HEAD
  ↓ filter
frontend/src/views/**/*.vue
```

**不**扫 `views/table/*`、`views/permission/*` 等历史 demo（既有 view 不进 diff 就不报）。

### 3.3 覆盖判定

对每个新增 .vue，在其所在目录（含任意子目录）查找 `*.test.{ts,tsx}`：

```
views/login/index.vue
  → views/login/*.test.ts        ✓ 覆盖
  → views/login/utils/*.test.ts  ✓ 覆盖
  → views/login/hooks/foo.test.ts ✓ 覆盖（任意子目录）
  → 无 .test.ts                    ✗ 报错
```

判定「同目录或任意子目录」：递归 `readdir` 看是否有 `.test.{ts,tsx}` 存在。**不要求文件名匹配 view 名**（AGENTS.md 允许 `utils/adapter.test.ts` 测 `index.vue`）。

### 3.4 输出格式

对齐现有 `check-*.js` 风格：

```
✗ views/feature/x/index.vue: 缺单测（no *.test.{ts,tsx} in views/feature/x/）
✗ views/foo/index.vue: 缺单测（no *.test.{ts,tsx} in views/foo/）

Scanned 5 new .vue files
❌ 2 view(s) without adjacent test
   fix: 加 <name>.test.ts 同目录，或 views/<svc>/utils/<adapter>.test.ts
```

退出码：`0` = 全部覆盖；`1` = 有缺。

### 3.5 不在 git repo / 无 base commit 时

- 无 `.git` 目录：`console.error("✗ not a git repo")` + 退出 1
- base commit 不存在（如 `git diff HEAD~1` 在 shallow clone 里）：`console.error` + 退出 1

---

## 4. 接入

### 4.1 改动清单

| 文件 | 改动 |
|---|---|
| `scripts/check-vitest-coverage.js` | 新增（~80 行） |
| `frontend/package.json` | 加 `"check:vitest-coverage": "node ../scripts/check-vitest-coverage.js"` |
| `docs/superpowers/specs/2026-07-08-frontend-check-vitest-coverage-design.md` | 本 spec |

### 4.2 使用方式

```bash
# 本地：相对 main
node scripts/check-vitest-coverage.js main

# 相对上一次 commit
node scripts/check-vitest-coverage.js HEAD~1

# CI：传 PR base branch
VITEST_COVERAGE_BASE=origin/main node scripts/check-vitest-coverage.js

# frontend
pnpm --filter vue-pure-admin run check:vitest-coverage
```

### 4.3 风险与降级

- **风险 1**：base ref 在 CI 与本地不一致（CI 默认 `main`，本地可能用 `HEAD~1`）
  - 降级：CI 传 `VITEST_COVERAGE_BASE=origin/$CI_MERGE_REQUEST_TARGET_BRANCH_NAME`
- **风险 2**：开发者本地未 push，main 已前进，diff 含他人 commit
  - 降级：本地默认 `main`，可能误报他人 view；用户接受（CI 用 origin/main 更准）
- **风险 3**：git 命令在 Windows PowerShell 上行为差异
  - 现有 check-*.js 全部用 `git` 命令（`git ls-files` 等），未发现平台 bug；本文同样

---

## 5. 测试矩阵（dry-run 验证）

| 场景 | 期望 |
|---|---|
| 临时建 `views/__lint_sentinel__/x.vue` 不带 test | 报错 + 退出 1 |
| 临时建 `views/__lint_sentinel__/x.vue` + `views/__lint_sentinel__/x.test.ts` | 通过 + 退出 0 |
| 临时建 `views/__lint_sentinel__/x.vue` + `views/__lint_sentinel__/utils/y.test.ts` | 通过 + 退出 0（子目录也算） |
| 无 git repo | 报错 `not a git repo` + 退出 1 |
| base ref 不存在 | 报错 + 退出 1 |

验证后清理 `views/__lint_sentinel__/`。

---

## 6. 验收

- [ ] `scripts/check-vitest-coverage.js` 存在 + 顶部 `#!/usr/bin/env node` + CommonJS 风格对齐现有 `check-*.js`
- [ ] `frontend/package.json` 加 `check:vitest-coverage` script
- [ ] 5 个 dry-run 场景全过
- [ ] 不扫已存在的 view（144 个历史 view 0 报错）
- [ ] 1 commit，含 spec + 脚本 + package.json

---

## 7. 不在本文 commit 范围

- 接入 `pnpm run check` 串
- 接入 pre-commit hook（`.husky/pre-commit`）
- 接入 `.gitlab-ci.yml` test 阶段
- 扫历史 view（143 个 view 一次性爆炸，不在「护航未来变更」范畴）

未来若要加，按 `frontend_todo.md §4.2` 立项即可。

---

_最后更新：2026-07-08_

# frontend typescript strict adoption plan

## Status (2026-07-09)

tsconfig.json 当前配置：

```jsonc
{
  "strict": false,
  "strictFunctionTypes": false,
  "noImplicitThis": true
}
```

项目处于开发阶段，目标是渐进式开启 strict mode，避免一次性爆炸。

## Baseline error count

| 模式 | 错误数 |
|---|---|
| 当前（baseline） | 0（typecheck 通过） |
| `noImplicitAny: true` only | **531** |
| `strict: true`（含 `strictNullChecks`） | **873** |

## 错误分布（`noImplicitAny` 模式下）

| top 10 文件 | 错误数 | 类型 |
|---|---:|---|
| `src/layout/components/lay-tag/index.vue` | 29 | pure-admin layout |
| `src/components/ReSeamlessScroll/src/index.vue` | 26 | pure-admin component |
| `src/components/ReSelector/src/index.tsx` | 25 | pure-admin component |
| `src/layout/hooks/useTag.ts` | 22 | pure-admin hook |
| `src/utils/chinaArea.ts` | 21 | pure-admin util |
| `src/layout/components/lay-search/components/SearchModal.vue` | 14 | pure-admin layout |
| `src/layout/components/lay-setting/index.vue` | 13 | pure-admin layout |
| `src/views/table/edit/demo2/columns.tsx` | 12 | **pure-admin demo** |
| `src/components/RePureTableBar/src/bar.tsx` | 11 | pure-admin component |
| `src/views/table/edit/demo3/columns.tsx` | 10 | **pure-admin demo** |

**结论**：大部分错误集中在纯 vue-pure-admin 上游组件 + demo views（已从 vitest-coverage 排除），非项目自写代码。

## 分阶段推进（推荐顺序）

### Phase 1: 隔离 pure-admin demo views（立即）

在 `tsconfig.json` 加 `exclude`：

```jsonc
{
  "exclude": [
    "node_modules",
    "dist",
    // 与 check-vitest-coverage.js 同一白名单：pure-admin 模板演示页
    "src/views/about",
    "src/views/able",
    "src/views/empty",
    "src/views/error",
    "src/views/guide",
    "src/views/list",
    "src/views/menuoverflow",
    "src/views/nested",
    "src/views/result",
    "src/views/table",
    "src/views/tabs"
  ]
}
```

**预期：错误数 531 → ~200**（剩下都是 pure-admin 上游组件代码）

### Phase 2: 渐进开启 strict 子项

每次只开一个子项 + 修一个区域：

1. `noImplicitAny: true` — 从 `src/api/` `src/store/` `src/utils/` 开始修（项目自写代码）
2. `strictNullChecks: true` — 同上，逐一处理
3. `strictFunctionTypes: true`
4. `alwaysStrict: true`（最终汇总开 `strict: true`）

每 phase 一个 commit，逐 review。

### Phase 3: 处理 pure-admin 上游组件

剩余 ~200 错误全在 `src/components/Re*` `src/layout/**`。选项：

- **A. 改源码**（5+ 天工作）
- **B. 升级 pure-admin 版本**（如果有修复过的 strict 兼容版本）
- **C. 局部加 `// @ts-nocheck`**（不推荐，会掩盖未来问题）
- **D. 仅对项目自写目录开 strict，pure-admin 目录保留**（折中）

## 暂缓标准

任何 phase 中如发现：
- 错误数 > 50 且分散到 > 20 文件 → 拆 sub-PR
- 错误集中在某 pure-admin 组件 → 跳过该组件，标 TODO，等上游

## 测试策略

- 每次开 strict 子项前先 `pnpm run typecheck` baseline 过
- 开后立刻 typecheck 拿错误数
- 修复按目录 batch：每目录 fix 完跑 typecheck + test:unit
- 全过才 commit

## 相关 issue / 上下文

- `frontend/AGENTS.md`：测试规范（每 view ≥ 1 happy-path）
- `frontend_todo.md §C 测试覆盖率提升`：29 → 100 已有 `check:vitest-coverage.js` 守护
- 当前 commit `96493be` feat(scripts): add --all report mode 加固覆盖率守卫
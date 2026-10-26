# PRISM 交互组件集 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 补齐壳骨架与业务页面所需的交互组件（弹框、抽屉、下拉、标签页、开关、勾选、Toast、Tooltip、Popover、命令面板、面包屑、头像、分页器、滚动区、分隔线），全部按 PRISM 视觉与可访问性规范实现。

**Architecture:** 交互能力用 **reka-ui 2.11**（无头原语，负责焦点陷阱、键盘导航、ARIA）；视觉由本项目自己的 scoped CSS + PRISM token 覆盖 reka-ui 内部结构（`:deep()`）。命令面板无对应原语，自研。

**Tech Stack:** Vue 3.5 / reka-ui 2.11 / vitest 2.1 + happy-dom + @vue/test-utils 2.4

**Spec:** `docs/superpowers/specs/2026-10-25-prism-admin-ui-design.md`；视觉与交互真源 `D:\Workspace\web-concept\design\phase1-visual-direction.md`（PRISM v2.0）与 `phase2-ux-spec.md`（Phase 2 v1.1）

## 共享契约（所有组件必须遵守）

1. **目录形态**：`frontend/src/components/ui/<kebab-name>/`，内含 `<Name>.vue`、`index.ts`（barrel）、`<Name>.test.ts`。
2. **样式**：一律写在组件 `<style scoped>` 内；reka-ui 内部元素用 `:deep()` 选择器；**颜色/尺寸/时长只能引用 token 变量，禁止裸 hex / rgba / 裸 px**（`scripts/check-prism-tokens.js` 阻断）。
3. **不依赖全局样式**：组件自带焦点环（`var(--size-focus-ring)` + 光晕）与读屏专用文本（`.ui-sr-only`），不依赖 `base.css`（该文件在 Plan 4 前尚未被引入）。
4. **可访问性**：ARIA 语义交给 reka-ui，不重复实现；`prefers-reduced-motion` 逐项降级（Phase 2 §5.4）。
5. **i18n**：组件内不硬编码中文；需要文案的位置接收 props 或走 slot。
6. **测试**：每个组件 ≥1 条 vitest 用例，`// @vitest-environment happy-dom`；断言**行为**（role / aria / 状态 / 事件），不只断言类名。
7. **禁止**：不使用 element-plus、不使用 pure-admin 的 `@pureadmin/*` 组件。

## 组件清单

| # | 组件 | 底座 | PRISM 规格出处 |
|---|---|---|---|
| 1 | Dialog 弹框 | reka-ui Dialog | §6.4 浮层、Phase 2 §5.2 焦点陷阱 |
| 2 | Sheet 抽屉（移动端侧栏） | reka-ui Dialog + 侧边变体 | Phase 2 §4.2 <640 抽屉 |
| 3 | Select 下拉 | reka-ui Select | §6.4 下拉菜单 |
| 4 | DropdownMenu 菜单 | reka-ui DropdownMenu | §6.4 |
| 5 | Tabs 标签页 | reka-ui Tabs | PRISM 补充设计（tabs 栏）+ 页内切换 |
| 6 | Switch 开关 | reka-ui Switch | §6.5 |
| 7 | Checkbox 勾选 | reka-ui Checkbox | Phase 1R 控件语言 |
| 8 | Toast 通知 | reka-ui Toast | Phase 2 §6.3（白卡 + 左侧 3px 色条） |
| 9 | Tooltip 提示 | reka-ui Tooltip | §6 浮层规格 |
| 10 | Popover 气泡 | reka-ui Popover | §6 浮层规格 |
| 11 | Command 命令面板 | 自研 | Phase 2 §3.2 / §5.2 |
| 12 | Breadcrumb 面包屑 | 自研 | Phase 2 §3.2 |
| 13 | Avatar 头像 | reka-ui Avatar | Phase 1R §6.9（28px 圆） |
| 14 | Pagination 分页器 | 自研 | Phase 2 §3.3 |
| 15 | ScrollArea 滚动区 | reka-ui ScrollArea | — |
| 16 | Separator 分隔线 | reka-ui Separator | Phase 1R §2.4 |

## 验收

- [ ] `pnpm -C frontend exec vitest run src/components/ui` 全绿
- [ ] `node scripts/check-prism-tokens.js` 退出码 0
- [ ] `pnpm -C frontend typecheck` 无错误
- [ ] `pnpm -C frontend exec eslint --max-warnings 0 src/components/ui` 无输出
- [ ] 每个组件在 `base.css` 未加载时仍具备可见焦点环
- [ ] Dialog / Sheet 打开时焦点入内、Esc 关闭、关闭后焦点还给触发器

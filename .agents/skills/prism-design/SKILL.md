---
name: prism-design
description: |
  PRISM 设计系统与前端代码规范约束。写任何前端代码（组件、页面、样式）前加载。
  Use when the user asks to write/modify frontend UI, add a component, style a page,
  or when an AI agent is about to create .vue / .css files under frontend/.
  覆盖 token 唯一来源、组件选型、可访问性、响应式降级链、文案语气与质量门禁。
---

# prism-design Skill

约束所有 PRISM 前端代码的写法。**在写任何 `.vue` / `.css` 之前先读这个 skill。**

## 1. 视觉数值的唯一来源

所有颜色、间距、字号、圆角、阴影、时长、缓动**只能**来自
`frontend/src/styles/prism/tokens.css`。

> token 背后的规范原文在 `docs/prism/`（Phase 1R 视觉规范 + Phase 2 体验规范）。
> 需要知道"这个数值为什么这么定"时读那里，不要猜。
> `tokens.test.ts` 直接解析那份规范校验 `tokens.css`——**改数值先改规范**。

| 类别 | 变量族 | 例 |
|---|---|---|
| 表面 | `--surface-*` | `--surface-base`（画布）、`--surface-raised`（白卡）、`--surface-sunken`（井/表头）、`--surface-ink`（墨色块） |
| 文字 | `--text-*` | `--text-primary` / `--text-secondary` / `--text-tertiary` / `--text-disabled` / `--text-inverse` |
| 边框 | `--border-*` | `--border-subtle` / `--border-default` / `--border-strong` / `--border-data` / `--border-accent-glow` |
| 主色 | `--accent-*` | `--accent`、`--accent-hover`、`--accent-pressed`、`--accent-deep`、`--accent-subtle`、`--accent-gradient` |
| 语义 | `--success-*` / `--warning-*` / `--danger-*` / `--info-*` | 每个语义三档：默认 / `-strong`（tint 底上的文字）/ `-subtle`（底色） |
| 阴影 | `--shadow-xs/sm/md/lg/accent/accent-lg` | E1 白卡 / E2 浮层 / E3 弹框 |
| 字阶 | `--font-size-*` / `--line-height-*` / `--weight-*` / `--tracking-*` | 按 `--font-size-<level>` 取值，字重用 `--weight-regular/medium/caption/body-strong/semibold/bold` |
| 尺寸 | `--size-*` | 控件高 `--size-control-sm/md/lg`、行高 `--size-row`、表头 `--size-header-cell`、侧栏 `--size-sidebar`、顶栏 `--size-topbar` |
| 动效 | `--duration-instant/fast/medium/slow`、`--ease-decelerate/standard` | 全站只有两支缓动曲线 |

**禁止**：在组件里写裸 hex / `rgb()` / `rgba()` / 裸 px / 裸 ms。
`scripts/check-prism-tokens.js` 会阻断（CI job `prism_tokens_check`）。

需要新数值时：**先回视觉侧确认**，再在 `tokens.css` 补充「PRISM 补充 token」段并写明来源章节。

## 2. 组件选型（不要另起一套）

已有基座在 `frontend/src/components/ui/`，全部基于 reka-ui + scoped CSS：

`Button` `Input` `Textarea` `Label` `FormField` `Card` `Badge` `Skeleton` `Table`
`Dialog` `Sheet` `Select` `DropdownMenu` `Tabs` `Switch` `Checkbox` `Toast`/`ToastHost`
`Tooltip` `Popover` `Command` `Breadcrumb` `Avatar` `Pagination` `ScrollArea` `Separator`

页面级两种形态（**只有这两种**，用户明确过）：

| 形态 | 组件 | 位置 |
|---|---|---|
| 列表页 | `shell/components/DataTable.vue` | `frontend/src/shell/components/` |
| 弹框编辑页 | `shell/components/FormDialog.vue` | 同上 |

写页面时**先用这两个**，不要自己造表格与弹框。

## 3. 关键视觉规则（Phase 1R / Phase 2）

- **hover 是加灰不是提亮**：一律 `--surface-hover-veil`；按下 `--surface-active-veil`。
- **tint 底上的文字用强调档**：选中行、侧栏当前项、Badge 在 `--accent-subtle` / `--*-subtle` 底上
  一律用 `--accent-deep` / `--*-strong`，**不要**用 `--accent` / `--*-` 默认档。
- **accent 实底永远配白字**（`--accent-contrast-text`），禁止在 accent 实底上放 secondary 灰字。
- **墨色面板（`--surface-ink`）全页占比 < 10%**，只用于页脚 / CTA 横幅，且永远平面无阴影。
- **大面积不铺 accent**：超过 120×120px 降级为 `--accent-subtle`。
- **信息类语义色是青绿 `--info`**，不要用 accent 蓝表达「提示」。
- **阴影三档对应三档层级**：E1 白卡 `shadow-xs`、E2 浮层 `shadow-md`、E3 弹框 `shadow-lg`。
- **动效是物理反馈不是表演**：hover 150ms、面板切换 240ms、路由/弹框 320ms（上限）。

## 4. 可访问性基线（Phase 2 §5 / §9.4）

- 焦点环：`outline: var(--size-focus-ring) solid var(--accent)` + offset，`:focus-visible` 才显示。
- 组件**必须自带焦点环**，不依赖全局 `base.css`（该文件在切换前可能没被引入）。
- 弹框 / 抽屉：焦点入内、Tab 循环、Esc 关闭、关闭后焦点还给触发器（reka-ui 已处理，别自己写）。
- 危险操作不能只靠颜色表达 —— 加 `.ui-sr-only` 补充文本或 `dangerLabel`。
- 表格用真语义：`<table>` + `<caption>` + `<th scope="col">`，排序用 `aria-sort`。
- 表单错误：`role="alert"` + 控件 `aria-invalid` + `aria-describedby` 指向错误文案。
- 图标按钮必须有 `aria-label`；纯装饰图标 `aria-hidden="true"`。
- `prefers-reduced-motion: reduce` 时逐项降级（Phase 2 §5.4 的十项清单）。

## 5. 响应式降级链（Phase 2 §4）

| 断点 | 侧栏 | 表格 | 指标卡 |
|---|---|---|---|
| <640 | 抽屉（`Sheet`） | 卡片化 | 1 列 |
| ≥640 | 抽屉 | 横向滚动 | 2 列 |
| ≥768 | 图标栏 64px | 横向滚动 | 2 列 |
| ≥1024 | 完整 240px（用户可折叠，localStorage 记忆） | 横向滚动 | 4 列 |

多标签页栏是**桌面专属**，`<768` 隐藏。

## 6. 文案语气（Phase 2 §8）

- 按钮/标题/导航/表头/Badge **不加句号**；正文与说明句加句号。
- 中文用直角引号「」，不用弯引号。
- 界面数字一律半角 + 等宽 + `tabular-nums`；≥5 位数用逗号千分位。
- 按钮写动词短语（2–6 字）：「保存」「新建部署」「重试」，不写「点击这里」。
- 错误 = 一句原因 + 一个动作，不以「抱歉」开头。
- 空态 = 陈述事实 + 一句说明 + 一个动作。

## 7. 质量门禁（必须过）

| 门禁 | 命令 | 要求 |
|---|---|---|
| 单测 | `pnpm -C frontend vitest run` | 每个新视图 ≥1 条；断言行为（role/aria/状态/事件），不只断言类名 |
| token 约束 | `node scripts/check-prism-tokens.js` | 退出码 0 |
| 类型 | `pnpm -C frontend typecheck` | 无错误 |
| 规范 | `pnpm -C frontend exec eslint --max-warnings 0 src/...` | 无输出 |
| i18n | `node scripts/check-i18n-hardcode.js` | 视图禁止硬编码中文；文案进 `locales/*.yaml` 的 `shell:` 或业务命名空间 |

## 8. 组件写法范式

```vue
<script setup lang="ts">
import { computed } from "vue";
import { cn } from "@/lib/cn";
import { Button } from "@/components/ui/button";

interface Props { variant?: "primary" | "secondary" | "ghost" }
const props = withDefaults(defineProps<Props>(), { variant: "primary" });
const classes = computed(() => cn("ui-x", `ui-x--${props.variant}`));
</script>

<template>
  <div :class="classes"><slot /></div>
</template>

<style scoped>
/* 数值一律引用 token */
.ui-x { background: var(--surface-raised); border-radius: var(--radius-md); }
</style>
```

目录约定：`frontend/src/components/ui/<kebab>/` 内放 `<Name>.vue` + `index.ts` + `<Name>.test.ts`。

## 9. 白标（换肤）

品牌覆盖只改 **`frontend/src/styles/prism/tokens.brand.css`** 一个文件（覆盖 `--accent-*`
一族 + 阴影/光晕）。覆盖后必须仍满足对比度断言 —— `tokens.test.ts` 会按有效值计算 WCAG
对比度，不达标即红。**不要**去改 `tokens.css`（那是 PRISM 默认色板，被契约测试锁定）。

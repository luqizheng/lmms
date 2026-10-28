# PRISM 设计系统基座 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 在上游 `frontend/` 内建立 PRISM 设计系统的可执行基座——token 层、排版基线、核心组件集、以及防止 AI 自造数值的校验器——不依赖任何业务页面，可独立测试通过。

**Architecture:** `frontend/src/styles/prism/` 承载 token 层（`:root` 变量）与排版基线；`frontend/src/components/ui/<kebab>/` 承载组件，每个组件一个目录（`.vue` + `index.ts` + `*.test.ts`），样式一律写在组件 `<style scoped>` 内并只引用 token 变量，**禁止裸 hex**；`scripts/check-prism-tokens.js` 扫描 `frontend/src`，拦截裸 hex 与未定义 CSS 变量引用。

**Tech Stack:** Vue 3.5 / Vite / TypeScript / vitest 2.1 + happy-dom + @vue/test-utils 2.4；reka-ui（交互组件，Plan 2 引入）；clsx + tailwind-merge（`cn` 工具）。

**Spec:** `docs/superpowers/specs/2026-10-25-prism-admin-ui-design.md`（视觉与交互真源另见 `D:\Workspace\web-concept\design\phase1-visual-direction.md` 与 `phase2-ux-spec.md`）

> **2026-10-28 注（追写，不改上文原文）**：上面引用的 `D:\Workspace\web-concept\design\...` 已迁入本仓库 [`docs/prism/`](../../prism/README.md) 作为权威副本；该目录降级为设计侧草稿区。详见 `docs/superpowers/specs/2026-10-28-prism-spec-versioning-design.md`。

## 执行对账（2026-10-26 已执行完毕）

本计划 8 个任务全部完成，产出在分支 `feat/prism-admin-ui`。下列偏离是执行中做出的裁决，**以本节为准**（下方任务正文保留原始形态，仅供追溯）：

| # | 偏离 | 理由 | 代价 |
|---|---|---|---|
| 1 | **不建 `theme.css`（Tailwind `@theme` 映射）** | `@theme` 与 `:root` 同名变量会自引用（`--shadow-xs` 等），且颜色走 utility 就无法被裸色值校验器拦截 | 页面级布局仍用 Tailwind utility；颜色一律 scoped CSS + token 变量 |
| 2 | **字阶 token 拆为四族**：`--font-size-*` / `--line-height-*` / `--weight-*` / `--tracking-*` | 原计划的 `--text-<level>` font 简写会在尺寸覆盖类中连带重置字重（sm 尺寸下 600 权重丢失） | 组件需写 4 行排版属性 |
| 3 | `tokens.css` 补入 **Phase 1R §3.2/§3.3 字阶** 与 **§4.2 间距阶梯** | 附录 CSS 速查未收录这两组，组件需要唯一真源 | 无 |
| 4 | 补充 token 增补 `--skeleton-shimmer` | §7.1 骨架屏 shimmer 色在正文内联，裸 rgba 会被校验器拦截 | 无 |
| 5 | token 契约测试用 `process.cwd()` 定位文件 | happy-dom 环境下 `import.meta.url` 非 file 协议 | 无 |
| 6 | 变体常量文件 `button.css.ts` → `button.ts` | `.css.ts` 易被误认为样式文件 | 无 |
| 7 | **Table 根元素为横向滚动包裹 DIV** | Phase 2 §4.2 要求 ≥640 表格横向滚动；测试断言相应改为校验内层 `table` | 调用方需注意根元素是 div |
| 8 | CI 接入方式：新增独立阻断 job `prism_tokens_check` | 不改动既有 `frontend_lint`（`allow_failure: true`）的语义，避免影响无关 lint | 多一个 CI job |
| 9 | 校验器排除 `*.test.ts`、`tokens.css` 允许裸色值；新增 `ALLOWED_VAR_PREFIXES` 白名单 | 测试文件与 token 定义处本身含 hex 是正常的 | 第三方库变量需在白名单登记 |
| 10 | `FormField` 中 `error` 优先于 `description`，二者不同时渲染 | 错误态下同时展示说明会误导 | 无 |

**已知遗留**：`frontend/src/views/notification/admin/utils/hook.test.ts` 在 main 分支上即已失败（视图新增第 5 列后测试未同步），与本计划无关，该目录将在 Plan 7 随 pure-admin 一并删除。

## 评审修复轮（2026-10-26，整分支评审后）

独立评审 agent 对 `main...feat/prism-admin-ui` 做了整分支评审，发现 2 个 Critical + 11 个 Important。已全部修复（每项均先写复现测试并观察其失败）：

| 编号 | 问题 | 修复 |
|---|---|---|
| C1 | `tokens.css` 漏抄附录成员 `--focus-glow-ink`，且"逐字转录"未被断言保护 | 补齐该 token；新增**附录变量集合断言**与**逐个值等价断言**（忽略空白，规避 prettier 规范化） |
| C2 | 对比度护栏根本没测对比度；且换肤时该测试会挡住换肤 | 新增 `tokens.brand.css` 品牌覆盖层（默认空，业务系统只改这一个文件）；新增按**有效值**计算的 WCAG 对比度断言（正文/accent/语义色/墨底），换肤后依然生效 |
| I1 | 组件里仍有裸尺寸与时长（44px 行高、1600ms shimmer、scale(0.98)…） | 补 `--size-*` / `--duration-shimmer` / `--duration-spinner` / `--scale-press` 等补充 token，组件全部改用 token |
| I2 | 表头字重实为 400，规范是 450 | 补 `--weight-caption: 450` 与 `--weight-body-strong: 550`，表头改用 450 档 |
| I3 | 校验器可被 `red` / `hsl()` / `oklch()` 绕过 | 校验器扩展：具名色黑名单 + 全部颜色函数正则；只对颜色类属性检查具名色以避免误报 |
| I4 | 校验器对跨行块注释误报（阻断式 CI 会假红） | 改为逐字符剥离注释并保留换行，行号不变 |
| I5 | 前端单测从不进 CI —— 计划声称的"测试覆盖"在 CI 里不生效 | 新增阻断式 job `frontend_test`（`pnpm -C frontend run test:unit`） |
| I6 | 组件依赖尚未引入的全局 `base.css`（caption 变可见、Input 失去焦点环） | Table 自带 `.ui-sr-only`；Input/Textarea 自带 outline + 光晕，不依赖全局样式 |
| I7 | reduced-motion 只降级了全局时长，未逐项覆盖 | Button（取消按压缩放、停 spinner）与 Skeleton（静止色块）各自加 `@media (prefers-reduced-motion: reduce)` |
| I8 | TableHeader 无 `aria-sort`、排序键盘不可达 | 新增 `sort` prop 输出 `aria-sort`；`sortable` 时内容渲染为可聚焦按钮 |
| I9 | 表单错误与控件无程序化关联 | FormField 通过 provide 暴露错误 id；Input/Textarea 自动获得 `aria-describedby` 与 `aria-invalid` |
| I10 | 校验器扫描范围靠人记得扩展 | 扫描根与 CI rules 集中声明；`tokens.brand.css` 与 `tokens.css` 一并列入定义处白名单 |
| I11 | 一批"只断言类名"的弱断言 | 新增 `ui.contract.test.ts`：直接断言组件样式源码中的关键声明，删样式即红 |
| M1–M6 | Button 缺顶部 inset 高光 / Input 错误态缺常驻光晕 / 表头字重 / 行 hover 分隔线 / spinner 颜色 / label 字重 | 全部补齐（Button 高光用 `--surface-highlight` token） |
| M7 | `rgba()` 内空格与附录非逐字 | 改为**值等价**断言（prettier 会规范化空格，字面逐字不可持续），已在上文说明 |
| M8 | 路线图仍写"Tailwind 映射" | 路线图交付物描述已同步 |
| 既有失败 | `notification/admin/utils/hook.test.ts` 在 main 即失败（列数 5≠4） | 测试名说"includes 4 base columns"但断言写成"恰好 4 列"；改为 `arrayContaining` 断言基础四列，保留原意。阻断式单测 CI 需要它绿 |

新增文件：`scripts/check-prism-tokens.test.js`（校验器自测，11 例）——校验器本身是护栏，必须有测试。

## Global Constraints

以下约束对每个任务生效，任务内不再重复：

1. **视觉数值唯一来源**：所有颜色、间距、字号、圆角、阴影、时长、缓动来自 `frontend/src/styles/prism/tokens.css`。**组件源码禁止出现任何裸 hex 色值**（`#RGB`/`#RRGGBB`/`rgb()`/`rgba()` 除非值来自 token 变量）。
2. **PRISM Phase 1R 附录**是 token 权威；Phase 1R 正文中内联的数值（如按钮 hover 渐变终点）必须**提取为补充 token**并标注来源，不得留在组件里。
3. **补充 token 命名**：`--focus-glow`、`--accent-gradient-hover` 等，统一加注释 `/* PRISM 补充 token — 来源：Phase 1R §x.y */`。
4. **组件样式一律 scoped CSS**，不使用 Tailwind utility 拼装颜色/圆角/阴影（避免与 PRISM token 走两条路径）。Tailwind 仍可用于页面级布局，且其 4px 间距基数与 PRISM 一致。
5. **组件源码零硬编码中文**：所有可见文案走 i18n key（本计划组件多为基础原语，文案集中在组件内 `t()` 调用处，留待 Plan 4 的 i18n 接入统一实现；本计划测试断言用 key 而非文案）。
6. **每个组件 ≥1 条 vitest 用例**，文件与组件同目录（`X.test.ts`）。
7. **纯增量**：本计划不修改 `frontend/src/style/`、`frontend/src/main.ts`、`frontend/index.html` 及任何现有 vue 文件，不改变现有应用运行结果。

## Review Focus

下列失败模式由 spec 隐含、但任务测试不天然覆盖，是最可能伤到使用者的。每一项都在对应任务里有专属断言：

1. **白标换肤后对比度崩坏** —— 品牌 accent 被替换后，文字对比度掉到 4.5:1 以下 → Task 1 的 token 契约测试锁定品牌关键色值。
2. **AI 新写组件时自造色值** —— 出现裸 hex 或引用了 tokens.css 里不存在的变量 → Task 8 的校验器拦截。
3. **组件在无 token 环境（未引入 prism 样式）下渲染成无样式** —— Task 1 的测试断言 token 文件被测试环境正确加载或至少结构完整。
4. **禁用态仍可点击 / loading 态重复提交** —— Task 3 的 Button 测试断言 `disabled` 与 `loading` 下的 DOM 属性。
5. **表格数值列不用等宽数字导致跳动** —— Task 6 的 Table 测试断言数值单元格带 mono/tabular-nums 类。
6. **表单错误文案缺失或不可被读屏播报** —— Task 7 的 FormField 测试断言 `role="alert"` 与错误文案存在。

---

### Task 1: PRISM token 层与排版基线

**Files:**
- Create: `frontend/src/styles/prism/tokens.css`
- Create: `frontend/src/styles/prism/base.css`
- Create: `frontend/src/styles/prism/index.css`
- Create: `frontend/src/styles/prism/tokens.test.ts`

**Interfaces:**
- Consumes: 无（首个任务）
- Produces:
  - CSS 变量清单（供 Task 8 校验器读取 `frontend/src/styles/prism/tokens.css`）
  - `frontend/src/styles/prism/index.css` — 供后续组件与壳通过 `import "@/styles/prism/index.css"` 引入

- [ ] **Step 1: 写失败的 token 契约测试**

创建 `frontend/src/styles/prism/tokens.test.ts`：

```ts
// @vitest-environment happy-dom
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";

const css = readFileSync(resolve(__dirname, "tokens.css"), "utf8");

function token(name: string): string {
  const m = css.match(new RegExp(`--${name}:\\s*([^;]+);`));
  return m ? m[1].trim() : "";
}

describe("PRISM token 契约", () => {
  it("品牌关键色值被锁定", () => {
    expect(token("accent")).toBe("#2E5BFF");
    expect(token("accent-hover")).toBe("#3D67FF");
    expect(token("accent-pressed")).toBe("#1F44D6");
    expect(token("accent-deep")).toBe("#2447CC");
    expect(token("accent-subtle")).toBe("#EBF0FF");
  });

  it("surface 三层暖灰阶梯被锁定", () => {
    expect(token("surface-sunken")).toBe("#F0F0EC");
    expect(token("surface-base")).toBe("#F7F7F4");
    expect(token("surface-raised")).toBe("#FFFFFF");
    expect(token("surface-ink")).toBe("#17181C");
  });

  it("文字四档 + 反色被锁定", () => {
    expect(token("text-primary")).toBe("#17181C");
    expect(token("text-secondary")).toBe("#52535B");
    expect(token("text-tertiary")).toBe("#6A6B73");
    expect(token("text-disabled")).toBe("#9C9DA5");
    expect(token("text-inverse")).toBe("#FFFFFF");
  });

  it("圆角 / 间距 / 时长 / 缓动阶梯齐全", () => {
    for (const name of [
      "radius-xs", "radius-sm", "radius-md", "radius-lg", "radius-xl", "radius-full",
      "space-1", "space-2", "space-3", "space-4", "space-5", "space-6",
      "space-8", "space-10", "space-12", "space-16", "space-20", "space-24",
      "duration-instant", "duration-fast", "duration-medium", "duration-slow",
      "ease-decelerate", "ease-standard"
    ]) {
      expect(token(name), `缺少 token --${name}`).not.toBe("");
    }
  });

  it("阴影六档齐全", () => {
    for (const name of ["shadow-xs", "shadow-sm", "shadow-md", "shadow-lg", "shadow-accent", "shadow-accent-lg"]) {
      expect(token(name), `缺少 token --${name}`).not.toBe("");
    }
  });

  it("每个变量定义都在 :root 作用域内且以分号结尾", () => {
    const declarations = css.match(/--[a-z0-9-]+\s*:/g) ?? [];
    const malformed = css.replace(/--[a-z0-9-]+\s*:[^;]+;/g, "");
    expect(malformed).not.toMatch(/--[a-z0-9-]+\s*:/);
    expect(declarations.length).toBeGreaterThan(50);
  });
});
```

- [ ] **Step 2: 运行测试确认失败**

Run: `cd frontend && pnpm vitest run src/styles/prism/tokens.test.ts`
Expected: FAIL —— `ENOENT` 找不到 `tokens.css`

- [ ] **Step 3: 创建 tokens.css（Phase 1R 附录转录 + 补充 token）**

创建 `frontend/src/styles/prism/tokens.css`。**主体**：逐字转录 PRISM Phase 1R 文档「附录：CSS 变量速查」中的 `:root { ... }` 全文（源：`D:\Workspace\web-concept\design\phase1-visual-direction.md`，附录节）。变量名与数值一字不改。

在 `:root` 末尾追加补充 token（值取自 Phase 1R 正文内联数值，**不得改动数值本身**）：

```css
  /* PRISM 补充 token — 来源：Phase 1R §6.1 按钮五态 */
  --accent-gradient-hover: linear-gradient(180deg, #3D67FF 0%, #2E5BFF 100%);
  --accent-gradient-pressed: linear-gradient(180deg, #1F44D6 0%, #1940BE 100%);
  --border-hover-strong: #B9B9B1;
  --surface-disabled: #EFEFEB;
  /* PRISM 补充 token — 来源：Phase 1R §7.1 骨架屏 */
  --skeleton-base: #FAFAF8;
  /* PRISM 补充 token — 来源：Phase 1R §5.5 焦点环 / §6.2 输入框聚焦 */
  --focus-glow: rgba(46,91,255,0.18);
  --focus-glow-ink: rgba(46,91,255,0.32);
  --input-focus-glow: rgba(46,91,255,0.15);
  --danger-focus-glow: rgba(220,38,38,0.12);
```

同时在文件顶部写三行注释，说明：本文件是 PRISM token 唯一真源、变更须回视觉侧确认、补充 token 已逐条标注来源。

- [ ] **Step 4: 创建 base.css（排版基线）**

创建 `frontend/src/styles/prism/base.css`，内容：

```css
/* PRISM 排版基线 — 来源：Phase 1R §3 字体 / §5.5 焦点环 / §5.4 reduced-motion */

:root {
  color-scheme: light;
  font-family: var(--font-ui);
  font-size: 16px;
  line-height: 1.6;
  color: var(--text-primary);
  background-color: var(--surface-base);
  -webkit-font-smoothing: antialiased;
  -moz-osx-font-smoothing: grayscale;
}

*,
*::before,
*::after {
  box-sizing: border-box;
}

body {
  margin: 0;
  background-color: var(--surface-base);
  color: var(--text-primary);
  font-size: var(--text-body);
  line-height: 26px;
}

h1, h2, h3, h4 {
  margin: 0;
  font-weight: 600;
}

a {
  color: var(--accent);
  text-decoration: none;
}

a:hover {
  color: var(--accent-hover);
}

:focus-visible {
  outline: 2px solid var(--accent);
  outline-offset: 2px;
  box-shadow: 0 0 0 4px var(--focus-glow);
}

code, kbd, pre, samp {
  font-family: var(--font-mono);
}

.tabular-nums {
  font-variant-numeric: tabular-nums;
}

.sr-only {
  position: absolute;
  width: 1px;
  height: 1px;
  padding: 0;
  margin: -1px;
  overflow: hidden;
  clip: rect(0, 0, 0, 0);
  white-space: nowrap;
  border-width: 0;
}

@media (prefers-reduced-motion: reduce) {
  *,
  *::before,
  *::after {
    animation-duration: 0.08s !important;
    animation-iteration-count: 1 !important;
    transition-duration: 0.08s !important;
    scroll-behavior: auto !important;
  }
}
```

- [ ] **Step 5: 创建 index.css 汇总入口**

创建 `frontend/src/styles/prism/index.css`：

```css
@import "./tokens.css";
@import "./base.css";
```

- [ ] **Step 6: 运行测试确认通过**

Run: `cd frontend && pnpm vitest run src/styles/prism/tokens.test.ts`
Expected: PASS —— 6 个用例全绿

- [ ] **Step 7: 提交**

```bash
git add frontend/src/styles/prism
git commit -m "feat(prism): PRISM token 层与排版基线"
```

---

### Task 2: cn 工具与基础依赖

**Files:**
- Create: `frontend/src/lib/cn.ts`
- Create: `frontend/src/lib/cn.test.ts`
- Modify: `frontend/package.json`（新增 dependencies）

**Interfaces:**
- Consumes: 无
- Produces: `cn(...inputs: ClassValue[]): string` —— 所有组件用于合并外部传入 class 的统一入口

- [ ] **Step 1: 写失败的测试**

创建 `frontend/src/lib/cn.test.ts`：

```ts
import { describe, expect, it } from "vitest";
import { cn } from "./cn";

describe("cn", () => {
  it("拼接多个 class", () => {
    expect(cn("a", "b")).toBe("a b");
  });

  it("忽略 falsy 值", () => {
    expect(cn("a", false, undefined, null, "", "b")).toBe("a b");
  });

  it("支持数组与对象条件", () => {
    expect(cn(["a", "b"], { c: true, d: false })).toBe("a b c");
  });

  it("后写的冲突 Tailwind 类覆盖先写的", () => {
    expect(cn("px-2", "px-4")).toBe("px-4");
  });
});
```

- [ ] **Step 2: 运行测试确认失败**

Run: `cd frontend && pnpm vitest run src/lib/cn.test.ts`
Expected: FAIL —— 无法解析 `./cn`

- [ ] **Step 3: 安装依赖**

Run: `cd frontend && pnpm add clsx tailwind-merge`
Expected: `package.json` 新增两项 dependencies，`pnpm-lock.yaml` 更新

- [ ] **Step 4: 实现 cn**

创建 `frontend/src/lib/cn.ts`：

```ts
import { clsx, type ClassValue } from "clsx";
import { twMerge } from "tailwind-merge";

export function cn(...inputs: ClassValue[]): string {
  return twMerge(clsx(inputs));
}

export type { ClassValue };
```

- [ ] **Step 5: 运行测试确认通过**

Run: `cd frontend && pnpm vitest run src/lib/cn.test.ts`
Expected: PASS —— 4 个用例全绿

- [ ] **Step 6: 提交**

```bash
git add frontend/src/lib frontend/package.json frontend/pnpm-lock.yaml
git commit -m "feat(prism): cn 工具与基础依赖"
```

---

### Task 3: Button 组件

**Files:**
- Create: `frontend/src/components/ui/button/Button.vue`
- Create: `frontend/src/components/ui/button/button.css.ts`（变体 class 名常量）
- Create: `frontend/src/components/ui/button/Button.test.ts`
- Create: `frontend/src/components/ui/button/index.ts`

**Interfaces:**
- Consumes: `cn`（Task 2）、token 变量（Task 1）
- Produces:
  ```ts
  export type ButtonVariant = "primary" | "secondary" | "ghost";
  export type ButtonSize = "sm" | "md" | "lg" | "icon";
  export interface ButtonProps {
    variant?: ButtonVariant;   // 默认 "primary"
    size?: ButtonSize;         // 默认 "md"
    loading?: boolean;         // 默认 false
    disabled?: boolean;        // 默认 false
    type?: "button" | "submit" | "reset";  // 默认 "button"
  }
  // 组件：<Button variant="ghost" size="sm" :loading="saving" @click="...">
  ```
  - 渲染根元素为原生 `<button>`；`loading` 时附加 `aria-busy="true"` 且 `disabled` 属性为真。

- [ ] **Step 1: 写失败的测试**

创建 `frontend/src/components/ui/button/Button.test.ts`：

```ts
// @vitest-environment happy-dom
import { describe, expect, it } from "vitest";
import { mount } from "@vue/test-utils";
import Button from "./Button.vue";

describe("Button", () => {
  it("默认渲染 primary 变体与 md 尺寸", () => {
    const w = mount(Button);
    expect(w.classes()).toContain("ui-button");
    expect(w.classes()).toContain("ui-button--primary");
    expect(w.classes()).toContain("ui-button--md");
  });

  it("透传 disabled 属性到原生 button", () => {
    const w = mount(Button, { props: { disabled: true } });
    expect(w.attributes("disabled")).toBeDefined();
  });

  it("loading 时禁用按钮并标记 aria-busy", () => {
    const w = mount(Button, { props: { loading: true } });
    expect(w.attributes("disabled")).toBeDefined();
    expect(w.attributes("aria-busy")).toBe("true");
    expect(w.classes()).toContain("is-loading");
  });

  it("非 loading 时不设置 aria-busy", () => {
    const w = mount(Button);
    expect(w.attributes("aria-busy")).toBeUndefined();
  });

  it("type 默认 button，可覆盖为 submit", () => {
    expect(mount(Button).attributes("type")).toBe("button");
    expect(mount(Button, { props: { type: "submit" } }).attributes("type")).toBe("submit");
  });

  it("variant 与 size 全部映射到约定类名", () => {
    expect(mount(Button, { props: { variant: "secondary" } }).classes()).toContain("ui-button--secondary");
    expect(mount(Button, { props: { variant: "ghost" } }).classes()).toContain("ui-button--ghost");
    expect(mount(Button, { props: { size: "sm" } }).classes()).toContain("ui-button--sm");
    expect(mount(Button, { props: { size: "lg" } }).classes()).toContain("ui-button--lg");
    expect(mount(Button, { props: { size: "icon" } }).classes()).toContain("ui-button--icon");
  });
});
```

- [ ] **Step 2: 运行测试确认失败**

Run: `cd frontend && pnpm vitest run src/components/ui/button/Button.test.ts`
Expected: FAIL —— 无法解析 `./Button.vue`

- [ ] **Step 3: 实现 Button.vue**

创建 `frontend/src/components/ui/button/button.css.ts`：

```ts
export const BUTTON_VARIANTS = ["primary", "secondary", "ghost"] as const;
export const BUTTON_SIZES = ["sm", "md", "lg", "icon"] as const;
export type ButtonVariant = (typeof BUTTON_VARIANTS)[number];
export type ButtonSize = (typeof BUTTON_SIZES)[number];
```

创建 `frontend/src/components/ui/button/Button.vue`：

```vue
<script setup lang="ts">
import { computed } from "vue";
import { cn } from "@/lib/cn";
import { BUTTON_SIZES, BUTTON_VARIANTS, type ButtonSize, type ButtonVariant } from "./button.css";

interface Props {
  variant?: ButtonVariant;
  size?: ButtonSize;
  loading?: boolean;
  disabled?: boolean;
  type?: "button" | "submit" | "reset";
}

const props = withDefaults(defineProps<Props>(), {
  variant: "primary",
  size: "md",
  loading: false,
  disabled: false,
  type: "button"
});

const isDisabled = computed(() => props.disabled || props.loading);
</script>

<template>
  <button
    :type="type"
    :disabled="isDisabled"
    :aria-busy="loading ? 'true' : undefined"
    :class="cn(
      'ui-button',
      `ui-button--${variant}`,
      `ui-button--${size}`,
      { 'is-loading': loading }
    )"
  >
    <span v-if="loading" class="ui-button__spinner" aria-hidden="true" />
    <slot />
  </button>
</template>

<style scoped>
.ui-button {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: var(--space-2);
  border: 1px solid transparent;
  font-family: var(--font-ui);
  font-size: var(--text-body);
  font-weight: 600;
  line-height: 1;
  border-radius: var(--radius-sm);
  padding: 0 var(--space-4);
  cursor: pointer;
  transition:
    background-color var(--duration-fast) var(--ease-decelerate),
    border-color var(--duration-fast) var(--ease-decelerate),
    color var(--duration-fast) var(--ease-decelerate),
    box-shadow var(--duration-fast) var(--ease-decelerate),
    transform var(--duration-instant) var(--ease-standard);
}

.ui-button:disabled {
  cursor: not-allowed;
}

/* Primary — Phase 1R §6.1 */
.ui-button--primary {
  background: var(--accent-gradient);
  color: var(--accent-contrast-text);
  box-shadow: var(--shadow-accent);
}
.ui-button--primary:hover:not(:disabled) {
  background: var(--accent-gradient-hover);
  box-shadow: var(--shadow-accent-lg);
}
.ui-button--primary:active:not(:disabled) {
  background: var(--accent-gradient-pressed);
  box-shadow: var(--shadow-accent);
  transform: scale(0.98);
}
.ui-button--primary:disabled {
  background: var(--surface-disabled);
  color: var(--text-disabled);
  box-shadow: none;
}

/* Secondary — Phase 1R §6.1 */
.ui-button--secondary {
  background: var(--surface-raised);
  border-color: var(--border-strong);
  color: var(--text-primary);
}
.ui-button--secondary:hover:not(:disabled) {
  background: var(--surface-hover-veil);
  border-color: var(--border-hover-strong);
}
.ui-button--secondary:active:not(:disabled) {
  background: var(--surface-sunken);
  transform: scale(0.98);
}
.ui-button--secondary:disabled {
  background: var(--surface-disabled);
  color: var(--text-disabled);
  border-color: transparent;
}

/* Ghost — Phase 1R §6.1 */
.ui-button--ghost {
  background: transparent;
  color: var(--text-secondary);
}
.ui-button--ghost:hover:not(:disabled) {
  background: var(--surface-hover-veil);
  color: var(--text-primary);
}
.ui-button--ghost:active:not(:disabled) {
  background: var(--surface-active-veil);
}
.ui-button--ghost:disabled {
  color: var(--text-disabled);
}

/* 尺寸系 — Phase 1R §6.1 */
.ui-button--sm {
  height: 32px;
  font-size: var(--text-small);
  padding: 0 var(--space-3);
}
.ui-button--md {
  height: 36px;
}
.ui-button--lg {
  height: 40px;
  font-size: var(--text-body-strong);
}
.ui-button--icon {
  width: 36px;
  height: 36px;
  padding: 0;
}

.ui-button__spinner {
  width: 14px;
  height: 14px;
  border: 2px solid currentcolor;
  border-right-color: transparent;
  border-radius: var(--radius-full);
  animation: ui-button-spin 800ms linear infinite;
}

@keyframes ui-button-spin {
  to {
    transform: rotate(360deg);
  }
}
</style>
```

- [ ] **Step 4: 创建 barrel 导出**

创建 `frontend/src/components/ui/button/index.ts`：

```ts
export { default as Button } from "./Button.vue";
export { BUTTON_SIZES, BUTTON_VARIANTS } from "./button.css";
export type { ButtonSize, ButtonVariant } from "./button.css";
```

- [ ] **Step 5: 运行测试确认通过**

Run: `cd frontend && pnpm vitest run src/components/ui/button/Button.test.ts`
Expected: PASS —— 6 个用例全绿

- [ ] **Step 6: 提交**

```bash
git add frontend/src/components/ui/button
git commit -m "feat(prism): Button 组件（primary/secondary/ghost × 五态）"
```

---

### Task 4: Input / Textarea / Label

**Files:**
- Create: `frontend/src/components/ui/input/Input.vue`
- Create: `frontend/src/components/ui/input/Input.test.ts`
- Create: `frontend/src/components/ui/input/index.ts`
- Create: `frontend/src/components/ui/textarea/Textarea.vue`
- Create: `frontend/src/components/ui/textarea/Textarea.test.ts`
- Create: `frontend/src/components/ui/textarea/index.ts`
- Create: `frontend/src/components/ui/label/Label.vue`
- Create: `frontend/src/components/ui/label/Label.test.ts`
- Create: `frontend/src/components/ui/label/index.ts`

**Interfaces:**
- Consumes: `cn`（Task 2）、token（Task 1）
- Produces:
  ```ts
  // Input
  export interface InputProps {
    modelValue?: string | number;
    type?: string;              // 默认 "text"
    placeholder?: string;
    disabled?: boolean;
    invalid?: boolean;          // 默认 false → 错误描边
    size?: "sm" | "md" | "lg";  // 默认 "md"
  }
  // 事件：update:modelValue(value: string)
  // 根元素 <input class="ui-input ...">，invalid 时附加 is-invalid

  // Textarea
  export interface TextareaProps {
    modelValue?: string;
    placeholder?: string;
    disabled?: boolean;
    invalid?: boolean;
    rows?: number;              // 默认 4
  }
  // 事件：update:modelValue(value: string)

  // Label
  export interface LabelProps {
    for?: string;               // 原生 for 属性
    required?: boolean;         // 默认 false → 追加 " *" 与 aria-hidden 标记
  }
  // 根元素 <label class="ui-label">
  ```

- [ ] **Step 1: 写三个组件的失败测试**

创建 `frontend/src/components/ui/input/Input.test.ts`：

```ts
// @vitest-environment happy-dom
import { describe, expect, it } from "vitest";
import { mount } from "@vue/test-utils";
import Input from "./Input.vue";

describe("Input", () => {
  it("双向绑定 modelValue", async () => {
    const w = mount(Input, { props: { modelValue: "a" } });
    expect((w.element as HTMLInputElement).value).toBe("a");
    await w.setValue("b");
    expect(w.emitted("update:modelValue")?.[0]).toEqual(["b"]);
  });

  it("invalid 时附加错误描边类", () => {
    expect(mount(Input).classes()).not.toContain("is-invalid");
    expect(mount(Input, { props: { invalid: true } }).classes()).toContain("is-invalid");
  });

  it("disabled 透传到原生元素", () => {
    expect(mount(Input, { props: { disabled: true } }).attributes("disabled")).toBeDefined();
  });
});
```

创建 `frontend/src/components/ui/textarea/Textarea.test.ts`：

```ts
// @vitest-environment happy-dom
import { describe, expect, it } from "vitest";
import { mount } from "@vue/test-utils";
import Textarea from "./Textarea.vue";

describe("Textarea", () => {
  it("双向绑定 modelValue", async () => {
    const w = mount(Textarea, { props: { modelValue: "a" } });
    expect((w.element as HTMLTextAreaElement).value).toBe("a");
    await w.setValue("b");
    expect(w.emitted("update:modelValue")?.[0]).toEqual(["b"]);
  });

  it("默认 rows 为 4", () => {
    expect(mount(Textarea).attributes("rows")).toBe("4");
  });

  it("invalid 时附加错误描边类", () => {
    expect(mount(Textarea, { props: { invalid: true } }).classes()).toContain("is-invalid");
  });
});
```

创建 `frontend/src/components/ui/label/Label.test.ts`：

```ts
// @vitest-environment happy-dom
import { describe, expect, it } from "vitest";
import { mount } from "@vue/test-utils";
import Label from "./Label.vue";

describe("Label", () => {
  it("透传 for 属性", () => {
    expect(mount(Label, { props: { for: "x" } }).attributes("for")).toBe("x");
  });

  it("required 时显示必填标记", () => {
    const w = mount(Label, { props: { required: true } });
    expect(w.text()).toContain("*");
    expect(w.classes()).toContain("is-required");
  });

  it("非 required 时无必填标记", () => {
    expect(mount(Label).text()).not.toContain("*");
  });
});
```

- [ ] **Step 2: 运行测试确认失败**

Run: `cd frontend && pnpm vitest run src/components/ui/input src/components/ui/textarea src/components/ui/label`
Expected: FAIL —— 三个组件文件均无法解析

- [ ] **Step 3: 实现 Label**

`frontend/src/components/ui/label/Label.vue`：

```vue
<script setup lang="ts">
import { computed } from "vue";
import { cn } from "@/lib/cn";

interface Props {
  for?: string;
  required?: boolean;
}

const props = withDefaults(defineProps<Props>(), { required: false });

const classes = computed(() => cn("ui-label", { "is-required": props.required }));
</script>

<template>
  <label :for="for" :class="classes">
    <slot />
    <span v-if="required" class="ui-label__required" aria-hidden="true">*</span>
  </label>
</template>

<style scoped>
.ui-label {
  display: inline-flex;
  align-items: center;
  gap: var(--space-1);
  font-size: var(--text-small);
  font-weight: 600;
  line-height: 20px;
  color: var(--text-primary);
}
.ui-label__required {
  color: var(--danger);
}
</style>
```

`frontend/src/components/ui/label/index.ts`：

```ts
export { default as Label } from "./Label.vue";
```

- [ ] **Step 4: 实现 Input**

`frontend/src/components/ui/input/Input.vue`：

```vue
<script setup lang="ts">
import { computed } from "vue";
import { cn } from "@/lib/cn";

interface Props {
  modelValue?: string | number;
  type?: string;
  placeholder?: string;
  disabled?: boolean;
  invalid?: boolean;
  size?: "sm" | "md" | "lg";
}

const props = withDefaults(defineProps<Props>(), {
  modelValue: "",
  type: "text",
  placeholder: undefined,
  disabled: false,
  invalid: false,
  size: "md"
});

const emit = defineEmits<{ "update:modelValue": [value: string] }>();

const classes = computed(() =>
  cn("ui-input", `ui-input--${props.size}`, { "is-invalid": props.invalid })
);

function onInput(event: Event) {
  emit("update:modelValue", (event.target as HTMLInputElement).value);
}
</script>

<template>
  <input
    :type="type"
    :value="modelValue"
    :placeholder="placeholder"
    :disabled="disabled"
    :class="classes"
    @input="onInput"
  />
</template>

<style scoped>
.ui-input {
  width: 100%;
  height: 36px;
  padding: 0 var(--space-3);
  font-family: var(--font-ui);
  font-size: var(--text-body);
  color: var(--text-primary);
  background: var(--surface-raised);
  border: 1px solid var(--border-default);
  border-radius: var(--radius-sm);
  transition:
    border-color var(--duration-fast) var(--ease-decelerate),
    box-shadow var(--duration-fast) var(--ease-decelerate);
}
.ui-input::placeholder {
  color: var(--text-tertiary);
}
.ui-input:hover:not(:disabled):not(.is-invalid) {
  border-color: var(--border-strong);
}
.ui-input:focus-visible {
  outline: none;
  border-color: var(--accent);
  box-shadow: 0 0 0 3px var(--input-focus-glow);
}
.ui-input.is-invalid {
  border-color: var(--danger);
}
.ui-input.is-invalid:focus-visible {
  box-shadow: 0 0 0 3px var(--danger-focus-glow);
}
.ui-input:disabled {
  background: var(--surface-base);
  border-color: var(--border-subtle);
  color: var(--text-disabled);
  cursor: not-allowed;
}
.ui-input--sm {
  height: 32px;
  font-size: var(--text-small);
}
.ui-input--lg {
  height: 40px;
}
</style>
```

`frontend/src/components/ui/input/index.ts`：

```ts
export { default as Input } from "./Input.vue";
```

- [ ] **Step 5: 实现 Textarea**

`frontend/src/components/ui/textarea/Textarea.vue`：

```vue
<script setup lang="ts">
import { computed } from "vue";
import { cn } from "@/lib/cn";

interface Props {
  modelValue?: string;
  placeholder?: string;
  disabled?: boolean;
  invalid?: boolean;
  rows?: number;
}

const props = withDefaults(defineProps<Props>(), {
  modelValue: "",
  placeholder: undefined,
  disabled: false,
  invalid: false,
  rows: 4
});

const emit = defineEmits<{ "update:modelValue": [value: string] }>();

const classes = computed(() => cn("ui-textarea", { "is-invalid": props.invalid }));

function onInput(event: Event) {
  emit("update:modelValue", (event.target as HTMLTextAreaElement).value);
}
</script>

<template>
  <textarea
    :value="modelValue"
    :placeholder="placeholder"
    :rows="rows"
    :disabled="disabled"
    :class="classes"
    @input="onInput"
  />
</template>

<style scoped>
.ui-textarea {
  width: 100%;
  padding: var(--space-2) var(--space-3);
  font-family: var(--font-ui);
  font-size: var(--text-body);
  line-height: 26px;
  color: var(--text-primary);
  background: var(--surface-raised);
  border: 1px solid var(--border-default);
  border-radius: var(--radius-sm);
  resize: vertical;
  transition:
    border-color var(--duration-fast) var(--ease-decelerate),
    box-shadow var(--duration-fast) var(--ease-decelerate);
}
.ui-textarea::placeholder {
  color: var(--text-tertiary);
}
.ui-textarea:hover:not(:disabled):not(.is-invalid) {
  border-color: var(--border-strong);
}
.ui-textarea:focus-visible {
  outline: none;
  border-color: var(--accent);
  box-shadow: 0 0 0 3px var(--input-focus-glow);
}
.ui-textarea.is-invalid {
  border-color: var(--danger);
}
.ui-textarea.is-invalid:focus-visible {
  box-shadow: 0 0 0 3px var(--danger-focus-glow);
}
.ui-textarea:disabled {
  background: var(--surface-base);
  border-color: var(--border-subtle);
  color: var(--text-disabled);
  cursor: not-allowed;
}
</style>
```

`frontend/src/components/ui/textarea/index.ts`：

```ts
export { default as Textarea } from "./Textarea.vue";
```

- [ ] **Step 6: 运行测试确认通过**

Run: `cd frontend && pnpm vitest run src/components/ui/input src/components/ui/textarea src/components/ui/label`
Expected: PASS —— 9 个用例全绿（3 + 3 + 3）

- [ ] **Step 7: 提交**

```bash
git add frontend/src/components/ui/input frontend/src/components/ui/textarea frontend/src/components/ui/label
git commit -m "feat(prism): Input / Textarea / Label 组件"
```

---

### Task 5: Card / Badge / Skeleton

**Files:**
- Create: `frontend/src/components/ui/card/Card.vue`
- Create: `frontend/src/components/ui/card/Card.test.ts`
- Create: `frontend/src/components/ui/card/index.ts`
- Create: `frontend/src/components/ui/badge/Badge.vue`
- Create: `frontend/src/components/ui/badge/Badge.test.ts`
- Create: `frontend/src/components/ui/badge/index.ts`
- Create: `frontend/src/components/ui/skeleton/Skeleton.vue`
- Create: `frontend/src/components/ui/skeleton/Skeleton.test.ts`
- Create: `frontend/src/components/ui/skeleton/index.ts`

**Interfaces:**
- Consumes: `cn`（Task 2）、token（Task 1）
- Produces:
  ```ts
  // Card
  export interface CardProps { interactive?: boolean; }   // 默认 false → hover 升边框+阴影
  // 具名插槽：header（标题区）、default（内容）、footer（底部）
  // 根元素 <section class="ui-card">

  // Badge
  export type BadgeVariant = "neutral" | "success" | "warning" | "danger" | "info" | "accent";
  export interface BadgeProps { variant?: BadgeVariant; }  // 默认 "neutral"
  // 根元素 <span class="ui-badge ui-badge--{variant}">

  // Skeleton
  export interface SkeletonProps { width?: string; height?: string; }  // 默认 undefined → 100% / 1em
  // 根元素 <div class="ui-skeleton" aria-hidden="true">
  ```

- [ ] **Step 1: 写三个组件的失败测试**

`frontend/src/components/ui/card/Card.test.ts`：

```ts
// @vitest-environment happy-dom
import { describe, expect, it } from "vitest";
import { mount } from "@vue/test-utils";
import Card from "./Card.vue";

describe("Card", () => {
  it("渲染 header / default / footer 三个插槽", () => {
    const w = mount(Card, {
      slots: { header: "<h3>标题</h3>", default: "<p>正文</p>", footer: "<span>底部</span>" }
    });
    expect(w.find(".ui-card__header").exists()).toBe(true);
    expect(w.find(".ui-card__body").exists()).toBe(true);
    expect(w.find(".ui-card__footer").exists()).toBe(true);
  });

  it("interactive 附加交互态类", () => {
    expect(mount(Card).classes()).not.toContain("is-interactive");
    expect(mount(Card, { props: { interactive: true } }).classes()).toContain("is-interactive");
  });

  it("无 footer 插槽时不渲染 footer 容器", () => {
    expect(mount(Card).find(".ui-card__footer").exists()).toBe(false);
  });
});
```

`frontend/src/components/ui/badge/Badge.test.ts`：

```ts
// @vitest-environment happy-dom
import { describe, expect, it } from "vitest";
import { mount } from "@vue/test-utils";
import Badge from "./Badge.vue";

describe("Badge", () => {
  it("默认 neutral 变体", () => {
    const w = mount(Badge);
    expect(w.classes()).toContain("ui-badge");
    expect(w.classes()).toContain("ui-badge--neutral");
  });

  it("六种变体均可渲染", () => {
    for (const v of ["neutral", "success", "warning", "danger", "info", "accent"]) {
      expect(mount(Badge, { props: { variant: v } }).classes()).toContain(`ui-badge--${v}`);
    }
  });
});
```

`frontend/src/components/ui/skeleton/Skeleton.test.ts`：

```ts
// @vitest-environment happy-dom
import { describe, expect, it } from "vitest";
import { mount } from "@vue/test-utils";
import Skeleton from "./Skeleton.vue";

describe("Skeleton", () => {
  it("对读屏隐藏", () => {
    expect(mount(Skeleton).attributes("aria-hidden")).toBe("true");
  });

  it("宽高可覆盖", () => {
    const w = mount(Skeleton, { props: { width: "120px", height: "20px" } });
    expect(w.attributes("style")).toContain("width: 120px");
    expect(w.attributes("style")).toContain("height: 20px");
  });
});
```

- [ ] **Step 2: 运行测试确认失败**

Run: `cd frontend && pnpm vitest run src/components/ui/card src/components/ui/badge src/components/ui/skeleton`
Expected: FAIL —— 三个组件文件均无法解析

- [ ] **Step 3: 实现 Card**

`frontend/src/components/ui/card/Card.vue`：

```vue
<script setup lang="ts">
import { computed } from "vue";
import { cn } from "@/lib/cn";

interface Props {
  interactive?: boolean;
}

const props = withDefaults(defineProps<Props>(), { interactive: false });

const classes = computed(() => cn("ui-card", { "is-interactive": props.interactive }));
</script>

<template>
  <section :class="classes">
    <header v-if="$slots.header" class="ui-card__header">
      <slot name="header" />
    </header>
    <div class="ui-card__body">
      <slot />
    </div>
    <footer v-if="$slots.footer" class="ui-card__footer">
      <slot name="footer" />
    </footer>
  </section>
</template>

<style scoped>
.ui-card {
  background: var(--surface-raised);
  border: 1px solid var(--border-default);
  border-radius: var(--radius-lg);
  box-shadow: var(--shadow-xs);
  transition:
    border-color var(--duration-fast) var(--ease-decelerate),
    box-shadow var(--duration-fast) var(--ease-decelerate);
}
.ui-card__header {
  padding: var(--space-6) var(--space-6) var(--space-4);
  border-bottom: 1px solid var(--border-subtle);
  font-size: var(--text-h4);
  font-weight: 600;
  line-height: 24px;
  color: var(--text-primary);
}
.ui-card__body {
  padding: var(--space-6);
}
.ui-card__footer {
  padding: var(--space-4) var(--space-6) var(--space-6);
  border-top: 1px solid var(--border-subtle);
}
.ui-card.is-interactive:hover {
  border-color: var(--border-strong);
  box-shadow: var(--shadow-sm);
}
</style>
```

`frontend/src/components/ui/card/index.ts`：

```ts
export { default as Card } from "./Card.vue";
```

- [ ] **Step 4: 实现 Badge**

`frontend/src/components/ui/badge/Badge.vue`：

```vue
<script setup lang="ts">
import { computed } from "vue";
import { cn } from "@/lib/cn";

export type BadgeVariant = "neutral" | "success" | "warning" | "danger" | "info" | "accent";

interface Props {
  variant?: BadgeVariant;
}

const props = withDefaults(defineProps<Props>(), { variant: "neutral" });

const classes = computed(() => cn("ui-badge", `ui-badge--${props.variant}`));
</script>

<template>
  <span :class="classes">
    <slot />
  </span>
</template>

<style scoped>
.ui-badge {
  display: inline-flex;
  align-items: center;
  height: 22px;
  padding: 0 var(--space-2);
  border-radius: var(--radius-full);
  font-size: var(--text-caption);
  font-weight: 500;
  letter-spacing: 0.02em;
  white-space: nowrap;
}
.ui-badge--neutral {
  background: var(--surface-sunken);
  color: var(--text-secondary);
}
.ui-badge--success {
  background: var(--success-subtle);
  color: var(--success-strong);
}
.ui-badge--warning {
  background: var(--warning-subtle);
  color: var(--warning-strong);
}
.ui-badge--danger {
  background: var(--danger-subtle);
  color: var(--danger-strong);
}
.ui-badge--info {
  background: var(--info-subtle);
  color: var(--info-strong);
}
.ui-badge--accent {
  background: var(--accent-subtle);
  color: var(--accent-deep);
  border: 1px solid var(--border-accent-glow);
}
</style>
```

`frontend/src/components/ui/badge/index.ts`：

```ts
export { default as Badge } from "./Badge.vue";
export type { BadgeVariant } from "./Badge.vue";
```

- [ ] **Step 5: 实现 Skeleton**

`frontend/src/components/ui/skeleton/Skeleton.vue`：

```vue
<script setup lang="ts">
import { computed } from "vue";

interface Props {
  width?: string;
  height?: string;
}

const props = withDefaults(defineProps<Props>(), { width: undefined, height: undefined });

const style = computed(() => ({
  width: props.width ?? "100%",
  height: props.height ?? "1em"
}));
</script>

<template>
  <div class="ui-skeleton" :style="style" aria-hidden="true" />
</template>

<style scoped>
.ui-skeleton {
  border-radius: var(--radius-xs);
  background-color: var(--skeleton-base);
  background-image: linear-gradient(
    90deg,
    transparent,
    rgba(23, 24, 28, 0.04),
    transparent
  );
  background-size: 200% 100%;
  animation: ui-skeleton-shimmer 1600ms linear infinite;
}

@keyframes ui-skeleton-shimmer {
  from {
    background-position: 200% 0;
  }
  to {
    background-position: -200% 0;
  }
}
</style>
```

`frontend/src/components/ui/skeleton/index.ts`：

```ts
export { default as Skeleton } from "./Skeleton.vue";
```

- [ ] **Step 6: 运行测试确认通过**

Run: `cd frontend && pnpm vitest run src/components/ui/card src/components/ui/badge src/components/ui/skeleton`
Expected: PASS —— 9 个用例全绿（3 + 2 + 2；Badge 变体在一个用例内循环断言）

- [ ] **Step 7: 提交**

```bash
git add frontend/src/components/ui/card frontend/src/components/ui/badge frontend/src/components/ui/skeleton
git commit -m "feat(prism): Card / Badge / Skeleton 组件"
```

---

### Task 6: Table 组件

**Files:**
- Create: `frontend/src/components/ui/table/Table.vue`
- Create: `frontend/src/components/ui/table/TableHead.vue`
- Create: `frontend/src/components/ui/table/TableBody.vue`
- Create: `frontend/src/components/ui/table/TableRow.vue`
- Create: `frontend/src/components/ui/table/TableHeader.vue`
- Create: `frontend/src/components/ui/table/TableCell.vue`
- Create: `frontend/src/components/ui/table/Table.test.ts`
- Create: `frontend/src/components/ui/table/index.ts`

**Interfaces:**
- Consumes: `cn`（Task 2）、token（Task 1）
- Produces:
  ```ts
  // 语义化真表格结构，类名约定：
  // <Table>            → <table class="ui-table">
  // <TableHead>        → <thead class="ui-table__head">
  // <TableBody>        → <tbody class="ui-table__body">
  // <TableRow>         → <tr class="ui-table__row">
  // <TableHeader>      → <th class="ui-table__th" scope="col">
  // <TableCell>        → <td class="ui-table__td">
  export interface TableCellProps {
    numeric?: boolean;   // 默认 false → 追加 is-numeric（mono + tabular-nums + 右对齐）
  }
  export interface TableRowProps {
    selected?: boolean;   // 默认 false → 追加 is-selected（accent-subtle 底 + 左缘 2px accent 竖条）
  }
  export interface TableHeaderProps {
    sortable?: boolean;   // 默认 false → 追加 is-sortable + aria-sort 由外部传入
  }
  // TableRow 额外暴露行 hover 态（CSS 实现，无需 JS）
  ```

- [ ] **Step 1: 写失败的测试**

`frontend/src/components/ui/table/Table.test.ts`：

```ts
// @vitest-environment happy-dom
import { describe, expect, it } from "vitest";
import { mount } from "@vue/test-utils";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow
} from "./index";

describe("Table", () => {
  it("渲染为真 table 结构并带 caption", () => {
    const w = mount(Table, {
      slots: { default: "<tbody><tr><td>x</td></tr></tbody>" }
    });
    expect(w.element.tagName).toBe("TABLE");
    expect(w.find("caption").exists()).toBe(true);
  });

  it("数值单元格带 mono 等宽数字类", () => {
    const w = mount(TableCell, { props: { numeric: true } });
    expect(w.classes()).toContain("is-numeric");
    expect(mount(TableCell).classes()).not.toContain("is-numeric");
  });

  it("选中行带选中态类", () => {
    const w = mount(TableRow, { props: { selected: true } });
    expect(w.classes()).toContain("is-selected");
  });

  it("表头默认 scope=col", () => {
    expect(mount(TableHeader).attributes("scope")).toBe("col");
  });

  it("可排序表头带 is-sortable 类", () => {
    expect(mount(TableHeader, { props: { sortable: true } }).classes()).toContain("is-sortable");
  });

  it("thead / tbody 语义正确", () => {
    expect(mount(TableHead).element.tagName).toBe("THEAD");
    expect(mount(TableBody).element.tagName).toBe("TBODY");
  });
});
```

- [ ] **Step 2: 运行测试确认失败**

Run: `cd frontend && pnpm vitest run src/components/ui/table`
Expected: FAIL —— 无法解析 `./index`

- [ ] **Step 3: 实现六个组件**

`Table.vue`：

```vue
<script setup lang="ts">
import { cn } from "@/lib/cn";

interface Props {
  caption?: string;
}

withDefaults(defineProps<Props>(), { caption: undefined });
</script>

<template>
  <div class="ui-table__wrapper">
    <table class="ui-table">
      <caption v-if="caption" class="sr-only">{{ caption }}</caption>
      <slot />
    </table>
  </div>
</template>

<style scoped>
.ui-table__wrapper {
  width: 100%;
  overflow-x: auto;
}
.ui-table {
  width: 100%;
  border-collapse: collapse;
  font-size: var(--text-body);
}
</style>
```

`TableHead.vue`：

```vue
<template>
  <thead class="ui-table__head">
    <slot />
  </thead>
</template>

<style scoped>
.ui-table__head {
  background: var(--surface-sunken);
}
</style>
```

`TableBody.vue`：

```vue
<template>
  <tbody class="ui-table__body">
    <slot />
  </tbody>
</template>
```

`TableRow.vue`：

```vue
<script setup lang="ts">
import { computed } from "vue";
import { cn } from "@/lib/cn";

interface Props {
  selected?: boolean;
}

const props = withDefaults(defineProps<Props>(), { selected: false });

const classes = computed(() => cn("ui-table__row", { "is-selected": props.selected }));
</script>

<template>
  <tr :class="classes">
    <slot />
  </tr>
</template>

<style scoped>
.ui-table__row {
  height: 44px;
  border-bottom: 1px solid var(--border-subtle);
  transition: background-color var(--duration-fast) var(--ease-decelerate);
}
.ui-table__row:hover {
  background-color: var(--surface-hover-veil);
}
.ui-table__row.is-selected {
  background-color: var(--accent-subtle);
  box-shadow: inset 2px 0 0 0 var(--accent);
}
</style>
```

`TableHeader.vue`：

```vue
<script setup lang="ts">
import { computed } from "vue";
import { cn } from "@/lib/cn";

interface Props {
  sortable?: boolean;
}

const props = withDefaults(defineProps<Props>(), { sortable: false });

const classes = computed(() => cn("ui-table__th", { "is-sortable": props.sortable }));
</script>

<template>
  <th scope="col" :class="classes">
    <slot />
  </th>
</template>

<style scoped>
.ui-table__th {
  height: 40px;
  padding: 0 var(--space-4);
  text-align: left;
  font-size: var(--text-caption);
  font-weight: 450;
  letter-spacing: 0.02em;
  color: var(--text-secondary);
  white-space: nowrap;
}
.ui-table__th.is-sortable {
  cursor: pointer;
  user-select: none;
}
</style>
```

`TableCell.vue`：

```vue
<script setup lang="ts">
import { computed } from "vue";
import { cn } from "@/lib/cn";

interface Props {
  numeric?: boolean;
}

const props = withDefaults(defineProps<Props>(), { numeric: false });

const classes = computed(() => cn("ui-table__td", { "is-numeric": props.numeric }));
</script>

<template>
  <td :class="classes">
    <slot />
  </td>
</template>

<style scoped>
.ui-table__td {
  padding: 0 var(--space-4);
  font-size: var(--text-body);
  color: var(--text-primary);
}
.ui-table__td.is-numeric {
  font-family: var(--font-mono);
  font-variant-numeric: tabular-nums;
  text-align: right;
}
</style>
```

- [ ] **Step 4: 创建 barrel 导出**

`frontend/src/components/ui/table/index.ts`：

```ts
export { default as Table } from "./Table.vue";
export { default as TableBody } from "./TableBody.vue";
export { default as TableCell } from "./TableCell.vue";
export { default as TableHead } from "./TableHead.vue";
export { default as TableHeader } from "./TableHeader.vue";
export { default as TableRow } from "./TableRow.vue";
```

- [ ] **Step 5: 运行测试确认通过**

Run: `cd frontend && pnpm vitest run src/components/ui/table`
Expected: PASS —— 6 个用例全绿

- [ ] **Step 6: 提交**

```bash
git add frontend/src/components/ui/table
git commit -m "feat(prism): Table 组件（44px 行高 / 语义化真表格）"
```

---

### Task 7: FormField 组件

**Files:**
- Create: `frontend/src/components/ui/form-field/FormField.vue`
- Create: `frontend/src/components/ui/form-field/FormField.test.ts`
- Create: `frontend/src/components/ui/form-field/index.ts`

**Interfaces:**
- Consumes: `cn`（Task 2）、token（Task 1）
- Produces:
  ```ts
  export interface FormFieldProps {
    label?: string;        // 字段名（i18n key 由调用方传入已翻译文案）
    required?: boolean;    // 默认 false
    description?: string;  // 说明文案
    error?: string;        // 错误文案；非空即进入错误态
    htmlFor?: string;      // 关联控件 id
  }
  // 结构：<div class="ui-form-field">
  //         <label class="ui-form-field__label">…</label>
  //         <slot />                              ← 控件
  //         <p class="ui-form-field__error" role="alert">…</p>   ← 仅 error 非空时渲染
  //         <p class="ui-form-field__description">…</p>          ← 仅 description 非空时渲染
  //       </div>
  // 错误态：根元素追加 is-invalid
  ```

- [ ] **Step 1: 写失败的测试**

`frontend/src/components/ui/form-field/FormField.test.ts`：

```ts
// @vitest-environment happy-dom
import { describe, expect, it } from "vitest";
import { mount } from "@vue/test-utils";
import FormField from "./FormField.vue";

describe("FormField", () => {
  it("渲染标签并关联控件", () => {
    const w = mount(FormField, { props: { label: "用户名", htmlFor: "u" } });
    expect(w.find("label").attributes("for")).toBe("u");
    expect(w.find("label").text()).toContain("用户名");
  });

  it("有 error 时渲染 role=alert 的错误文案", () => {
    const w = mount(FormField, { props: { error: "名称不能超过 24 个字符" } });
    const err = w.find(".ui-form-field__error");
    expect(err.exists()).toBe(true);
    expect(err.attributes("role")).toBe("alert");
    expect(err.text()).toBe("名称不能超过 24 个字符");
  });

  it("无 error 时不渲染错误文案元素", () => {
    expect(mount(FormField).find(".ui-form-field__error").exists()).toBe(false);
  });

  it("有 error 时根元素进入错误态", () => {
    expect(mount(FormField, { props: { error: "x" } }).classes()).toContain("is-invalid");
    expect(mount(FormField).classes()).not.toContain("is-invalid");
  });

  it("required 显示必填标记", () => {
    const w = mount(FormField, { props: { label: "邮箱", required: true } });
    expect(w.find(".ui-form-field__label").text()).toContain("*");
  });

  it("description 存在时渲染说明", () => {
    expect(mount(FormField, { props: { description: "用于登录" } }).find(".ui-form-field__description").exists()).toBe(true);
  });
});
```

- [ ] **Step 2: 运行测试确认失败**

Run: `cd frontend && pnpm vitest run src/components/ui/form-field`
Expected: FAIL —— 无法解析 `./FormField.vue`

- [ ] **Step 3: 实现 FormField**

`frontend/src/components/ui/form-field/FormField.vue`：

```vue
<script setup lang="ts">
import { computed } from "vue";
import { cn } from "@/lib/cn";

interface Props {
  label?: string;
  required?: boolean;
  description?: string;
  error?: string;
  htmlFor?: string;
}

const props = withDefaults(defineProps<Props>(), {
  label: undefined,
  required: false,
  description: undefined,
  error: undefined,
  htmlFor: undefined
});

const classes = computed(() => cn("ui-form-field", { "is-invalid": Boolean(props.error) }));
</script>

<template>
  <div :class="classes">
    <label v-if="label" class="ui-form-field__label" :for="htmlFor">
      <span>{{ label }}</span>
      <span v-if="required" class="ui-form-field__required" aria-hidden="true">*</span>
    </label>
    <slot />
    <p v-if="error" class="ui-form-field__error" role="alert">{{ error }}</p>
    <p v-else-if="description" class="ui-form-field__description">{{ description }}</p>
  </div>
</template>

<style scoped>
.ui-form-field {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
}
.ui-form-field__label {
  display: inline-flex;
  align-items: center;
  gap: var(--space-1);
  font-size: var(--text-small);
  font-weight: 600;
  line-height: 20px;
  color: var(--text-primary);
}
.ui-form-field__required {
  color: var(--danger);
}
.ui-form-field__error {
  margin: var(--space-1) 0 0;
  font-size: var(--text-caption);
  line-height: 18px;
  color: var(--danger);
}
.ui-form-field__description {
  margin: 0;
  font-size: var(--text-caption);
  line-height: 18px;
  color: var(--text-tertiary);
}
</style>
```

`frontend/src/components/ui/form-field/index.ts`：

```ts
export { default as FormField } from "./FormField.vue";
```

- [ ] **Step 4: 运行测试确认通过**

Run: `cd frontend && pnpm vitest run src/components/ui/form-field`
Expected: PASS —— 6 个用例全绿

- [ ] **Step 5: 提交**

```bash
git add frontend/src/components/ui/form-field
git commit -m "feat(prism): FormField 组件（标签/错误播报/说明）"
```

---

### Task 8: 设计约束校验器

**Files:**
- Create: `scripts/check-prism-tokens.js`
- Modify: `frontend/package.json`（新增 script `check:prism-tokens`）
- Modify: `.gitlab-ci.yml`（前端 job 增加门禁调用）

**Interfaces:**
- Consumes: `frontend/src/styles/prism/tokens.css`（Task 1）
- Produces: 可执行脚本 `node scripts/check-prism-tokens.js`，退出码 0 = 通过，1 = 违规；输出每条违规的 `文件:行` 与原因。

- [ ] **Step 1: 写脚本**

创建 `scripts/check-prism-tokens.js`：

```js
#!/usr/bin/env node
/**
 * PRISM 设计约束门禁。
 *
 * 1. 组件源码禁止裸 hex / rgb() 色值 —— 颜色只能来自 tokens.css。
 * 2. 源码中引用的 CSS 变量必须在 tokens.css 中已定义。
 *
 * 扫描范围：frontend/src/components/ui、frontend/src/styles/prism
 * 排除：tokens.css 自身（定义处）、base.css 中经允许的 rgba 字面量。
 */
const { readFileSync, readdirSync, statSync } = require("node:fs");
const { join, relative } = require("node:path");

const ROOT = join(__dirname, "..");
const FRONTEND_SRC = join(ROOT, "frontend", "src");
const TOKENS_FILE = join(FRONTEND_SRC, "styles", "prism", "tokens.css");
const SCAN_ROOTS = [
  join(FRONTEND_SRC, "components", "ui"),
  join(FRONTEND_SRC, "styles", "prism")
];

const tokens = readFileSync(TOKENS_FILE, "utf8");
const definedTokens = new Set(
  [...tokens.matchAll(/--([a-z0-9-]+)\s*:/g)].map((m) => m[1])
);

const HEX_RE = /#[0-9a-fA-F]{3,8}\b/g;
const RGB_RE = /\brgba?\(/g;
const VAR_RE = /var\(\s*--([a-z0-9-]+)/g;

function walk(dir, out = []) {
  let entries;
  try {
    entries = readdirSync(dir);
  } catch {
    return out;
  }
  for (const entry of entries) {
    const full = join(dir, entry);
    const st = statSync(full);
    if (st.isDirectory()) walk(full, out);
    else if (/\.(vue|css|ts|tsx)$/.test(entry)) out.push(full);
  }
  return out;
}

const violations = [];

for (const file of SCAN_ROOTS.flatMap((dir) => walk(dir))) {
  if (file === TOKENS_FILE) continue;
  const rel = relative(ROOT, file);
  const source = readFileSync(file, "utf8");
  const lines = source.split("\n");

  lines.forEach((line, index) => {
    const stripped = line.replace(/\/\/.*$/, "").replace(/\/\*.*?\*\//g, "");

    if (file.endsWith(".css") && file.includes("base.css")) {
      // base.css 的焦点环 rgba 已在 tokens.css 有 --focus-glow 覆盖，此处仅允许该文件的白名单
      return;
    }

    const hex = stripped.match(HEX_RE);
    if (hex) {
      violations.push(`${rel}:${index + 1} 裸色值 ${hex.join(", ")} —— 请改用 tokens.css 变量`);
    }
    const rgb = stripped.match(RGB_RE);
    if (rgb) {
      violations.push(`${rel}:${index + 1} 裸色值 ${rgb.join(", ")} —— 请改用 tokens.css 变量`);
    }
    for (const m of stripped.matchAll(VAR_RE)) {
      if (!definedTokens.has(m[1])) {
        violations.push(`${rel}:${index + 1} 引用了未定义变量 --${m[1]}`);
      }
    }
  });
}

if (violations.length > 0) {
  console.error("[check-prism-tokens] 违规 %d 条：", violations.length);
  for (const v of violations) console.error("  " + v);
  process.exit(1);
}

console.log(`[check-prism-tokens] 通过：tokens 已定义 ${definedTokens.size} 个`);
```

- [ ] **Step 2: 验证脚本能抓到违规**

临时在 `frontend/src/components/ui/button/Button.vue` 的样式里加一行 `background: #ff0000;`，运行：

Run: `node scripts/check-prism-tokens.js`
Expected: 退出码 1，输出包含 `裸色值 #ff0000` 与 `Button.vue:<行号>`

删除该临时行后重跑，Expected: 退出码 0，输出 `[check-prism-tokens] 通过`。

- [ ] **Step 3: 接入 package.json script**

在 `frontend/package.json` 的 `scripts` 中新增：

```json
"check:prism-tokens": "node ../scripts/check-prism-tokens.js"
```

- [ ] **Step 4: 接入 CI**

在 `.gitlab-ci.yml` 的 `frontend_lint:` job 中，`pnpm -C frontend run lint:eslint` 之后追加一行：

```yaml
    - pnpm -C frontend run check:prism-tokens
```

同时把该 job 的 `allow_failure: true` 改为 `false`（门禁需阻断）——若仓库 CI 约定为不阻断，则保留原值并在 job 内注释说明原因。

- [ ] **Step 5: 跑全量前端测试确认无回归**

Run: `cd frontend && pnpm vitest run`
Expected: 全部通过，既有测试（http/dashboard/menu 等）不受影响

- [ ] **Step 6: 提交**

```bash
git add scripts/check-prism-tokens.js frontend/package.json .gitlab-ci.yml
git commit -m "feat(prism): 设计约束校验器（禁裸色值 / 校验变量引用）"
```

---

## 完成后自查

- [ ] `cd frontend && pnpm vitest run` 全绿
- [ ] `node scripts/check-prism-tokens.js` 退出码 0
- [ ] `cd frontend && pnpm typecheck` 无错误
- [ ] `cd frontend && pnpm build` 成功
- [ ] 现有 pure-admin 应用仍可运行（`pnpm -C frontend dev` 无报错，本计划未触碰任何既有运行路径）

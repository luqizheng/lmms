# PRISM 控制台

公司自有的 admin 前端。**独立工程** —— 不基于任何后台模板框架（vue-pure-admin / element-plus / Tailwind 均已移除）。

## 技术栈

Vue 3.5 · Vite · TypeScript · [reka-ui](https://reka-ui.com)（无头原语）· Pinia · vue-i18n · lucide-vue-next · axios（封装在 `@digua/http`）

样式全部是 **scoped CSS + PRISM 设计 token**，不用 Tailwind。

## 目录

```
src/
  styles/prism/     设计系统 token 层（唯一真源）+ 品牌覆盖层
  components/ui/    组件基座
  lib/              通用工具
  shell/            壳：api / store / router / layout / views / i18n
locales/            i18n（shell: 与 system: 两个命名空间）
```

## 设计系统

视觉规范见 `docs/superpowers/specs/2026-10-25-prism-admin-ui-design.md`，
token 真源在 `src/styles/prism/tokens.css`（154 个变量，逐值对齐 PRISM Phase 1R 附录）。

**换肤**只改 `src/styles/prism/tokens.brand.css` 一个文件；覆盖后仍需满足
WCAG 对比度断言（`tokens.test.ts` 按有效值计算）。

## 命令

```bash
pnpm dev
pnpm build
pnpm test:unit
pnpm typecheck
pnpm lint:eslint
```

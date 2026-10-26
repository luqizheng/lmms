---
name: scaffold
description: |
  用 PRISM 壳初始化一个新的业务系统前端（拷贝壳 + 装基础包 + 品牌化）。
  Use when the user says "新起一个业务系统/新项目前端", "scaffold 一个前端",
  or asks to create a new admin frontend based on the shell.
---

# scaffold Skill

为一个新的业务系统生成可登录、可导航的前端骨架。

## 前提

- 壳的真源在上游仓库 `low-memory-micro-services`（本 skill 所在仓库）的 `frontend/`。
- 业务系统是**独立仓库、独立构建、独立部署**（含 air-gap）。不要做巨石前端 / 微前端聚合。
- 基础包 `@digua/http` 与 `@digua/contracts` 从内网 registry `http://npm.kudi.repos/` 安装。

## 第 1 步：确认输入

| 输入 | 说明 | 缺省 |
|---|---|---|
| 业务系统名 | 用于包名与标题 | 必填 |
| 产品名 | 顶栏与登录页显示 | 同业务系统名 |
| 后端地址前缀 | 请求 baseURL | `/api` |
| 品牌主色 | hex，用于生成 accent 色阶 | 用 PRISM 默认钴蓝 `#2E5BFF` |
| Logo | svg 或图片路径 | PRISM 棱镜标记 |
| 包 scope | npm 包名前缀 | `@<org>` |

品牌色缺省就**不要问**，用默认色并说明。

## 第 2 步：拷贝壳

壳的目录（在源仓库里）：

```
frontend/src/styles/prism/       ← token 层 + 品牌覆盖层（整体拷贝）
frontend/src/components/ui/      ← 组件基座（整体拷贝）
frontend/src/lib/               ← cn 工具
frontend/src/shell/              ← 壳本体：api / store / router / components / views / i18n
frontend/src/shell/test-utils.ts
frontend/locales/*.yaml          ← i18n（含 shell: 命名空间）
```

拷贝规则：

- **整体拷贝，不做挑选** —— 壳是整体演进的，挑选会让后续升级失配。
- `frontend/src/shell/views/` 下除 `login/` `dashboard/` `profile/` `NotFound.vue`
  `ComingSoon.vue` 外的业务视图**不拷贝**（那是框架自己的页面）。
- legacy 的 `frontend/src/views` `frontend/src/layout` `frontend/src/router`
  `frontend/src/store` `frontend/src/utils` **不拷贝**（pure-admin 残留，Plan 7 删除）。

## 第 3 步：装依赖与配置

`package.json` 需要：

```json
{
  "dependencies": {
    "@digua/contracts": "^0.1.0",
    "@digua/http": "^0.1.0",
    "vue": "^3.5.35",
    "vue-router": "^5.1.0",
    "pinia": "^3.0.4",
    "vue-i18n": "^11.4.4",
    "reka-ui": "^2.11.0",
    "lucide-vue-next": "^1.0.0",
    "axios": "^1.14.0"
  }
}
```

构建配置照抄源仓库的 `vite.config.ts` / `tsconfig.json` / `build/`，注意：

- alias：`@` → `src`，`@build` → `build`
- `types`: `["node", "vite/client"]`
- tsconfig 的 `include` 只覆盖存活代码（`src/shell/**`、`src/components/ui/**`、`src/lib/**`、`src/styles/**`）
- `tailwindcss` **不需要**（PRISM 组件用 scoped CSS + token，不用 Tailwind utility）

## 第 4 步：品牌化（只改 token 文件）

编辑 `src/styles/prism/tokens.brand.css`，覆盖 accent 一族：

```css
:root {
  --accent: #7C3AED;
  --accent-hover: #8B5CF6;
  --accent-pressed: #5B21B6;
  --accent-deep: #4C1D95;
  --accent-subtle: #F3EEFF;
  --accent-wash: rgba(124, 58, 237, 0.08);
  --accent-gradient: linear-gradient(180deg, #7C3AED 0%, #6D28D9 100%);
  --accent-gradient-hover: linear-gradient(180deg, #8B5CF6 0%, #7C3AED 100%);
  --accent-gradient-pressed: linear-gradient(180deg, #5B21B6 0%, #4C1D95 100%);
  --accent-disabled: rgba(124, 58, 237, 0.4);
  --border-accent-glow: rgba(124, 58, 237, 0.3);
  --focus-glow: rgba(124, 58, 237, 0.18);
  --input-focus-glow: rgba(124, 58, 237, 0.15);
  --shadow-accent: 0 1px 2px rgba(76, 29, 149, 0.24), 0 4px 12px rgba(124, 58, 237, 0.22);
  --shadow-accent-lg: 0 2px 4px rgba(76, 29, 149, 0.28), 0 8px 20px rgba(124, 58, 237, 0.28);
}
```

**对比度是硬门槛**：品牌色换完后跑

```bash
cd frontend && pnpm vitest run src/styles/prism/tokens.test.ts
```

该测试按**有效值**（默认 + 品牌覆盖）计算 WCAG 对比度：

- `accent` 作链接文字在 `--surface-raised` / `--surface-base` 上 ≥ 4.5:1
- `accent` 实底配白字 ≥ 4.5:1（渐变两端都要）
- `accent-deep` 在 `--accent-subtle` 上 ≥ 4.5:1

不达标就是品牌色不合适 —— 换更深的色，**不要**放宽断言。

## 第 5 步：接入后端

`src/shell/api/client.ts` 里：

- `baseURL` 改成实际后端地址（默认 `/api`）。
- `authWhiteList` 按后端公开端点补（登录、刷新、注册、setup-status）。
- 错误码文案：`translateError` 已经用 `@digua/contracts` 的中英文映射，无需改。

## 第 6 步：换标题与 Logo

- `index.html` 的 `<title>`。
- `src/shell/views/login/LoginPage.vue` 的棱镜标记（`.ui-login__mark`）与标题文案。
- 标题/副标题文案在 `locales/*.yaml` 的 `shell.login.*`（中文主界面，英文保持一致结构）。

## 第 7 步：验证

```bash
cd frontend
pnpm install
pnpm vitest run
pnpm exec eslint --max-warnings 0 src
npx vue-tsc --noEmit --skipLibCheck
pnpm build
cd .. && node scripts/check-prism-tokens.js
```

再人工确认：登录页视觉、登录后侧栏与顶栏、⌘K 命令面板、窄屏抽屉侧栏。

## 第 8 步：接 DB 菜单

新系统的菜单来自后端 `GET /v1/pure/get-async-routes`（不是前端写死路由）。
返回 `{ code, message, data: RouteNode[] }`，`data` 是**已激活**菜单配置版本里的路由树，
每个节点形如 `{ path, name, component, meta: { title, icon, showLink, ... }, children }`。

- 菜单可见性由 `meta.showLink` 决定（`false` 则不进侧栏，但路由仍注册、可直接访问）。
- 节点的 `component` 字段对应 `src/shell/views/<路径>`（如 `component: "system/user"`
  → `src/shell/views/system/user/index.vue`）。解析不到会显示「该页面正在迁移」占位。
- 菜单配置版本本身由 `/v1/pure/menu-configs` 管理（列表 / 新建 / 编辑 / publish / clone），
  与路由树装载是**两件事**。

详见 `gen-page` skill。

## 交付检查清单

- [ ] `pnpm build` 通过，产物能起
- [ ] 登录 → 跳转业务首页 → 侧栏菜单来自 DB
- [ ] 品牌色已换且对比度测试通过
- [ ] 窄屏（<640）侧栏变抽屉、表格卡片化
- [ ] ⌘K 命令面板能唤起
- [ ] `check-prism-tokens` 退出码 0

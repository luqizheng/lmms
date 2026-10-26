# frontend 前端服务规范

PRISM 控制台 —— 公司自有的 admin 前端。**独立工程**，不基于任何后台模板框架。

## 必需文件

- [x] Dockerfile
- [x] nginx.conf
- [x] package.json
- [x] README.md
- [x] 国际化配置（locales/）
- [x] 入口（index.html → src/shell/entry.ts）

## 配置选项

- VITE_PORT: 本地端口，默认 `8848`
- VITE_API_BASE: API 基础路径，默认 `/api`
- VITE_PUBLIC_PATH: 公共路径，默认 `/`
- VITE_ROUTER_HISTORY: 路由模式（hash）

## 项目结构

| 目录 | 说明 |
|------|------|
| `src/styles/prism/` | **PRISM 设计系统 token 层**（唯一真源，见下） |
| `src/components/ui/` | 组件基座（reka-ui + scoped CSS + token） |
| `src/lib/` | 通用工具（cn 等） |
| `src/shell/api/` | 后端接口封装（基于 `@digua/http`） |
| `src/shell/store/` | Pinia store（auth / menu） |
| `src/shell/router/` | 路由与鉴权守卫（DB 动态菜单装载） |
| `src/shell/components/` | 壳布局（侧栏 / 顶栏 / 标签栏 / 命令面板 / 页面形态） |
| `src/shell/views/` | 框架系统页 |
| `locales/` | i18n 资源（`shell:` 与 `system:` 两个命名空间） |

## 硬性约束

1. **视觉数值唯一来源** = `src/styles/prism/tokens.css`。
   组件源码禁止出现裸 hex / rgba / 裸 px / 裸 ms。
   门禁：`node ../scripts/check-prism-tokens.js`（CI 阻断）。
2. **颜色不得写死**。语义色用 `--success/--warning/--danger/--info`；
   tint 底上的文字用强调档（`--accent-deep` / `--*-strong`）。
3. **样式一律 scoped CSS**，不走 Tailwind utility 拼装颜色（Tailwind 未引入）。
4. **图标用 `lucide-vue-next`**，不引入图标字体或 SVG 雪碧图。
5. **文案不得硬编码中文**，统一走 `locales/*.yaml`。
   门禁：`node ../scripts/check-i18n-hardcode.js` 与 `node ../scripts/check-i18n-namespace.js`。
6. **每个视图 ≥1 条 vitest 用例**，断言行为而非类名。
   门禁：`node ../scripts/check-vitest-coverage.js`。
7. **写操作按钮必须带权限校验**：`v-if="auth.hasPermission('<权限码>')"`。
8. **组件必须自带焦点环**，不依赖全局样式；`prefers-reduced-motion` 逐项降级。

## 常用命令

```bash
pnpm dev                 # 开发
pnpm build               # 构建（会先构建 @digua/* 基础包）
pnpm test:unit           # 单测
pnpm typecheck           # 类型检查
pnpm lint:eslint         # 规范
```

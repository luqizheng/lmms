# Coder Micro Services 文档站

基于 [VitePress](https://vitepress.dev) 构建的文档门户，内容源是仓库根的 [`docs/`](../docs/) 目录。

## 设计原则

- **单一真相源**：`docs/` 是文档唯一来源，本目录通过 **symlink** 引入，不复制内容。改文档改 `docs/` 即可，文档站自动同步。
- **docs/ 零改动**：本目录只放 VitePress 配置（`.vitepress/config.ts`）、首页（`index.md`）和 symlink，不动 `docs/` 任何文件。
- **中文优先**：站点 `lang: zh-CN`，主题文案、搜索框全部本地化。

## 目录结构

```
docs-site/
├─ .vitepress/
│  └─ config.ts          # 站点配置（nav / sidebar / 搜索 / 中文主题）
├─ index.md              # 首页（hero + features）
├─ README.md             # 本文件
├─ ai-context.md         # → ../docs/ai-context.md (symlink)
├─ developer-workflow.md # → ../docs/developer-workflow.md (symlink)
├─ USER_MANUAL.md        # → ../docs/USER_MANUAL.md (symlink)
├─ onboarding/           # → ../docs/onboarding (symlink)
├─ user-manual/          # → ../docs/user-manual (symlink)
├─ guides/               # → ../docs/guides (symlink)
├─ decisions/            # → ../docs/decisions (symlink)
├─ runbooks/             # → ../docs/runbooks (symlink)
├─ superpowers/          # → ../docs/superpowers (symlink)
└─ plans/                # → ../docs/plans (symlink)
```

## 本地开发

依赖装在仓库根（`vitepress` + `vue` 在根 `package.json` 的 `devDependencies`）：

```bash
# 在仓库根
pnpm install                 # 安装 vitepress / vue
pnpm docs:dev                # 启动 dev server（默认 http://localhost:5173）
pnpm docs:build              # 构建静态站点到 docs-site/.vitepress/dist
pnpm docs:preview            # 预览构建产物
```

## Symlink 维护

symlink 使用相对路径（`../docs/...`），在 Linux / macOS / WSL2 下正常工作。如需重建：

```bash
cd docs-site
# 子目录
ln -sf ../docs/onboarding onboarding
ln -sf ../docs/user-manual user-manual
ln -sf ../docs/guides guides
ln -sf ../docs/decisions decisions
ln -sf ../docs/runbooks runbooks
ln -sf ../docs/superpowers superpowers
ln -sf ../docs/plans plans
# 顶级 md
ln -sf ../docs/ai-context.md ai-context.md
ln -sf ../docs/developer-workflow.md developer-workflow.md
ln -sf ../docs/USER_MANUAL.md USER_MANUAL.md
```

> Windows 原生 git 默认不解析 symlink；本项目要求 WSL2 开发，故 symlink 方案适用。

## 新增文档

1. 在 `docs/` 对应子目录新增 `.md` 文件（真相源）。
2. 在 `docs-site/.vitepress/config.ts` 的 `sidebar` 对应分组追加一条 `{ text, link }`。
3. `pnpm docs:dev` 验证。

## 部署

`pnpm docs:build` 产物在 `docs-site/.vitepress/dist/`，是纯静态文件，可托管到任意静态服务器 / GitLab Pages / Vercel / Netlify。

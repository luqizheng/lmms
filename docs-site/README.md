# Coder Micro Services 文档站

基于 [VitePress](https://vitepress.dev) 构建的文档门户。

## 内容来源（两种模式并存）

| 内容 | 存放位置 | 维护方式 |
|------|----------|----------|
| onboarding / user-manual / index.md | `docs-site/` 下直接维护 | 文档站专属内容，直接编辑 `docs-site/` 内文件 |
| guides / decisions / runbooks / plans / ai-context / developer-workflow | 仓库根 `docs/` | **单一真相源**，通过 symlink 引入，改 `docs/` 即自动同步 |

> 设计原则：`docs/` 放跨团队通用的工程文档（guides / ADR / runbooks），`docs-site/` 放面向读者的入门 / 手册内容。两边互不复制。

## 目录结构

```
docs-site/
├─ .vitepress/
│  └─ config.mts           # 站点配置（nav / sidebar / 搜索 / 中文主题 / rewrites）
├─ index.md                # 首页（hero + features）
├─ README.md               # 本文件（工程说明，srcExclude 不进文档站）
├─ package.json            # 占位（依赖装在仓库根 package.json）
├─ public/
│  └─ logo.svg             # 静态资源
├─ onboarding/             # 真实目录（文档站专属，直接维护）
├─ user-manual/            # 真实目录（文档站专属，直接维护）
├─ ai-context.md           # -> ../docs/ai-context.md            (symlink)
├─ developer-workflow.md   # -> ../docs/developer-workflow.md    (symlink)
├─ guides/                 # -> ../docs/guides                   (symlink)
├─ decisions/              # -> ../docs/decisions                (symlink)
├─ runbooks/               # -> ../docs/runbooks                 (symlink)
└─ plans/                  # -> ../docs/plans                    (symlink)
```

## 如何使用

### 环境准备

依赖（`vitepress` + `vue`）声明在仓库根 `package.json` 的 `devDependencies`，`docs-site/package.json` 仅占位。

```bash
# 在仓库根
pnpm install
```

### 本地开发

```bash
pnpm docs:dev       # 启动 dev server，默认 http://localhost:5173，支持热更新
pnpm docs:build     # 构建静态站点到 docs-site/.vitepress/dist/
pnpm docs:preview   # 预览构建产物（先 build 再 preview）
```

### 新增文档

**情况一：文档站专属内容（onboarding / user-manual）**

1. 在 `docs-site/onboarding/` 或 `docs-site/user-manual/` 下新增 `.md`。
2. 在 `.vitepress/config.mts` 的 `sidebar` 对应分组追加 `{ text, link }`。
3. `pnpm docs:dev` 验证页面可访问。

**情况二：通用工程文档（guides / decisions / runbooks / plans）**

1. 在 `docs/` 对应子目录新增 `.md`（真相源）。
2. 若是全新子目录，在 `docs-site/` 下建 symlink：`ln -sf ../docs/<new-dir> <new-dir>`。
3. 在 `.vitepress/config.mts` 的 `sidebar` 追加条目。
4. `pnpm docs:dev` 验证。

### 编写规范

- **中文优先**：站点 `lang: zh-CN`，正文用中文；代码 / 命令 / 路径 / 技术标识符保持英文。
- **frontmatter**：可选。`title` 覆盖页面标题；VitePress 自动取第一个 H1。
- **内部链接**：用相对路径（如 `[测试](./04-testing)`），不带 `.md` 后缀（`cleanUrls: true`）。`config.mts` 已设 `ignoreDeadLinks: true`，因 symlink 会让 dead-link 检测器路径前缀错位误报，实际生成的链接基于页面 URL 解析，正确。
- **图片**：放 `docs-site/public/`，引用用绝对路径 `/logo.svg`。

### 修改站点配置

编辑 `.vitepress/config.mts`，常用区域：

| 区域 | 作用 |
|------|------|
| `nav` | 顶部导航栏 |
| `sidebar` | 各 section 侧边栏（按 URL 前缀匹配） |
| `rewrites` | 把 `README.md` 映射为目录首页 `index.md` |
| `themeConfig.search` | 本地搜索（已中文化） |
| `srcExclude` | 排除不进文档站的文件（当前排除 `README.md`） |

## 如何发布

### 构建产物

```bash
pnpm docs:build
# 产物: docs-site/.vitepress/dist/  （纯静态 HTML/JS/CSS）
pnpm docs:preview   # 本地预览 dist/，确认无误再部署
```

`dist/` 是自包含的静态站点，可托管到任意静态文件服务器。

### 部署方式

#### 方式一：GitLab Pages（推荐，配合项目现有 CI）

在 `.gitlab-ci.yml` 追加 `pages` job：

```yaml
pages:
  stage: deploy
  image: node:20-alpine
  cache:
    key: docs-${CI_COMMIT_REF_SLUG}
    paths:
      - .pnpm-store/
  before_script:
    - corepack enable
    - pnpm config set store-dir .pnpm-store
    - pnpm install --frozen-lockfile
  script:
    - pnpm docs:build
    - mv docs-site/.vitepress/dist public
  artifacts:
    paths:
      - public
  rules:
    - if: $CI_COMMIT_BRANCH == $CI_DEFAULT_BRANCH
      changes:
        - docs/**/*
        - docs-site/**/*
        - package.json
        - pnpm-lock.yaml
  dependencies: []
```

推送 `main` 后，GitLab 自动构建并在 `Settings → Pages` 暴露站点 URL。

> symlink 注意：GitLab CI runner 在 Linux 上 checkout，symlink 正常解析。若用 Windows runner 需配置 `git config --global core.symlinks true`。

#### 方式二：Docker 镜像（nginx 托管）

`docs-site/Dockerfile` 已就绪：vitepress build 在宿主机完成（需要完整 git 仓库以生成 `lastUpdated`），Dockerfile 只打包预构建的 `dist/` 到 nginx。已接入项目镜像编排器，`pnpm run docker:build:docs-site` 会自动先跑 `pnpm docs:build` 再 `docker buildx build`。

```bash
# 单独构建 docs-site 镜像（自动预构建 + 本地留镜像，不推送）
pnpm run docker:build:docs-site

# 带版本号 + 推送到内网 registry
pnpm run docker:build:docs-site -- -v 1.0.0 --push

# 本地留镜像 + 推送
pnpm run docker:build:docs-site -- --load-and-push -v 1.0.0

# 多架构
pnpm run docker:build:docs-site -- --platform linux/amd64,linux/arm64 --push

# 或手动两步（构建上下文必须是仓库根）
pnpm docs:build
docker build -t docs-site:latest -f docs-site/Dockerfile .
docker run -p 8080:80 docs-site:latest
# 访问 http://localhost:8080
```

镜像推送到 `${REGISTRY}/micro-server/docs-site`，与其它服务镜像同仓库。`nginx.conf` 已配置 VitePress `cleanUrls` 兜底（`try_files $uri $uri/ $uri.html $uri/index.html`）、gzip、静态资源缓存与健康检查。

#### 方式三：直接拷贝静态文件

```bash
pnpm docs:build
rsync -avz docs-site/.vitepress/dist/ user@server:/var/www/docs/
```

### 发布检查清单

- [ ] `pnpm docs:build` 无报错
- [ ] `pnpm docs:preview` 本地浏览，关键页面可访问
- [ ] 新增的 sidebar 条目 link 与实际文件路径一致
- [ ] symlink 指向的 `docs/` 文件存在（`ls -L docs-site/<symlink>` 无 broken link）

## Symlink 维护

symlink 使用相对路径（`../docs/...`），在 Linux / macOS / WSL2 下正常工作。如需重建：

```bash
cd docs-site
ln -sf ../docs/ai-context.md ai-context.md
ln -sf ../docs/developer-workflow.md developer-workflow.md
ln -sf ../docs/guides guides
ln -sf ../docs/decisions decisions
ln -sf ../docs/runbooks runbooks
ln -sf ../docs/plans plans
```

> Windows 原生 git 默认不解析 symlink；本项目要求 WSL2 开发，故 symlink 方案适用。

## 故障排查

| 现象 | 原因 | 解决 |
|------|------|------|
| 页面 404 | sidebar link 与文件路径不匹配 | 核对 `config.mts` 中 link 与实际文件名（含中文），注意 `cleanUrls` 去掉了 `.md` |
| `pnpm docs:dev` 报 symlink 找不到 | docs/ 下源文件被删 / 移动 | 重建 symlink 或在 `config.mts` 移除对应 sidebar 条目 |
| 构建报 dead link | `ignoreDeadLinks: true` 在 dev 下不阻断，build 下也不阻断，但页面会显示断链 | 全局搜索 `(./` 检查相对链接 |
| 搜索框无结果 | 首次 build 后才生成搜索索引 | `pnpm docs:build` 后 `pnpm docs:preview` 验证 |
| sidebar 分组不显示 | URL 前缀与 sidebar key 不匹配 | sidebar key 是 URL 前缀（如 `/onboarding/`），需带尾斜杠 |

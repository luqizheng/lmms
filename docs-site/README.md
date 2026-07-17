# Coder Micro Services 文档站

基于 [VitePress](https://vitepress.dev) 构建的文档门户。

## 内容来源

| 内容 | 存放位置 | 维护方式 |
|------|----------|----------|
| 全部 VitePress 渲染内容（guides / decisions / runbooks / plans / onboarding / user-manual / ai-context.md / developer-workflow.md / USER_MANUAL.md / index.md） | `docs-site/` 下直接维护 | 文档站专属内容，物理上全部在 `docs-site/` 内 |
| 设计 spec / plan / 进度 | 仓库根 `docs/superpowers/` | 不进文档站，由 AI / 工程流程直接读 |
| 数据库脚本 | 仓库根 `docs/*.sql`, `docs/*.csv` | 不进文档站 |

> 之前的设计是「`docs/` 放跨团队通用的工程文档、`docs-site/` 放读者入门手册 + 通过 symlink 引用 `docs/`」；本次重整后所有文档站内容物理上落在 `docs-site/`（删 symlink），`docs/` 只保留 `superpowers/` specs + 数据库脚本。`docs/user-manual/` 作为历史备份保留（drift 已 reconcile：docs-site 版本含 `12-release-and-distribute.md`，docs 版本较旧）。

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
├─ onboarding/             # 真实目录
├─ user-manual/            # 真实目录
├─ guides/                 # 真实目录
├─ decisions/              # 真实目录
├─ runbooks/               # 真实目录
├─ plans/                  # 真实目录
├─ ai-context.md           # 真实文件
├─ developer-workflow.md   # 真实文件
└─ USER_MANUAL.md          # 真实文件
```

> 全部内容物理上落在 `docs-site/` 内，无 symlink。仓库根 `docs/` 只保留 `superpowers/` specs 与数据库脚本。

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

文档站所有内容都在 `docs-site/` 下，新增页面只需：

1. 在 `docs-site/<section>/` 下新增 `.md`（或直接在 `docs-site/` 根目录放顶级页）。
2. 若需要左侧 sidebar 入口，在 `.vitepress/config.mts` 的 `sidebar` 对应分组追加 `{ text, link }`。
3. 若需要顶部 nav 入口，在 `nav` 数组追加 `{ text, link }`。
4. `pnpm docs:dev` 验证页面可访问 + 链接生效。

> 历史指南（`情况二：通用工程文档（guides / decisions / runbooks / plans）`）描述的「在 `docs/` 加文件 + docs-site 建 symlink」流程已废弃——所有内容现在直接落 `docs-site/`。

### 编写规范

- **中文优先**：站点 `lang: zh-CN`，正文用中文；代码 / 命令 / 路径 / 技术标识符保持英文。
- **frontmatter**：可选。`title` 覆盖页面标题；VitePress 自动取第一个 H1。
- **内部链接**：用相对路径（如 `[测试](./04-testing)`），不带 `.md` 后缀（`cleanUrls: true`）。`config.mts` 已设 `ignoreDeadLinks: true`。
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

## 故障排查

## 故障排查

| 现象 | 原因 | 解决 |
|------|------|------|
| 页面 404 | sidebar link 与文件路径不匹配 | 核对 `config.mts` 中 link 与实际文件名（含中文），注意 `cleanUrls` 去掉了 `.md` |
| `pnpm docs:dev` 报 symlink 找不到 | 历史版本残留问题——symlink 已废弃，如重新出现请直接 rm 并确保文件物理上在 docs-site/ |
| 构建报 dead link | `ignoreDeadLinks: true` 在 dev 下不阻断，build 下也不阻断，但页面会显示断链 | 全局搜索 `(./` 检查相对链接 |
| 搜索框无结果 | 首次 build 后才生成搜索索引 | `pnpm docs:build` 后 `pnpm docs:preview` 验证 |
| sidebar 分组不显示 | URL 前缀与 sidebar key 不匹配 | sidebar key 是 URL 前缀（如 `/onboarding/`），需带尾斜杠 |

# @digua/dist-tools

发布与同步工具集。配合 [coder-micro-services](https://gitlab/team/low-memory-micro-services) 上游框架使用。

> **v0.2.0+ 完全用 Node.js 实现，跨 Linux / macOS / Windows**。不再依赖 bash。

## 解决什么问题

上游框架（`low-memory-micro-services`）发布新版本后，下游项目（如 `project-manager`）需要手动解包、对比、复制文件——这是个非常痛苦的过程。本包把这个流程自动化。

## 安装

```bash
pnpm add -D @digua/dist-tools
```

依赖：
- Node.js ≥ 18
- 可选：`git` CLI（用于三方合并，开发环境基本都有）

## 使用

### 1. 初始化同步策略

在下游项目根目录创建 `.upstream-sync.yaml`：

```yaml
managed:
  # 上游完全管理，直接覆盖
  - ".cargo/config.toml"
  - "pnpm-workspace.yaml"
  - "package.json"
  - "bin/*.sh"
  - "Makefile"
  - "README.md"

three-way-merge:
  # 双方都可能改，用 git merge-file 做三方合并
  - "contracts/**"
  - "scripts/**"
  - ".agents/**"
  - "infrastructure/**"
  - "Cargo.toml"

skip:
  # 永远不碰
  - ".gitignore"
  - ".env"
  - "apps/**"
  - "services/**"
  - "secrets/**"
  - ".upstream-sync.yaml"
  - ".upstream.lock"
```

### 2. 检查当前同步状态

```bash
npx cms-sync --check
```

### 3. 从上游 tarball 同步

```bash
# 干跑 — 只显示会做什么，不修改任何文件
npx cms-sync --dry-run ./coder-micro-services-v1.1.0-airgap.tar.gz

# 实际同步
npx cms-sync ./coder-micro-services-v1.1.0-airgap.tar.gz

# 直接拉 URL（HTTP/HTTPS）
npx cms-sync https://releases.example.com/v1.1.0.tar.gz
```

## 同步策略说明

| 策略 | 行为 | 适用场景 |
|------|------|----------|
| `managed` | 直接覆盖当前文件 | 上游完全管理的文件 |
| `three-way-merge` | 用 git merge-file 做三方合并 | 双方都可能修改的配置文件 |
| `skip` | 不动（但显式 `managed` 优先） | 下游独有的内容；通配排除可被 `managed` 显式覆盖 |
| `protected-managed` | dst 不变；上游版本另存为 `<file>.new.<ext>` | 双方都可能改且下游可能加业务内容（如 `docker-compose.yml` 通过 `include:` 加业务 compose）；由开发者手动 diff 合并 |

### 优先级与 skip 的语义

- `managed` 列表显式列出的文件**总是**被覆盖，即使 `skip` 通配也命中——显式 > 通配。
- `merge`（`three-way-merge`）文件如果被 `skip` 命中则被排除，不会被强制合并——防止"忘记下游改了"的意外覆盖。
- `skip` 只显式排除 `merge` 文件；`managed` 显式列出可破除 skip。
- 既不在 `managed` 也不在 `merge` 也不在 `skip` 的文件**隐式不动**（与显式 `skip` 行为相同，但 yaml 看不到）。
- 当一个文件同时出现在 `managed` 和 `skip` 时，sync 会 warn `N file(s) listed in BOTH managed and skip — managed wins`，方便排查策略冗余。

### `protected-managed` 的行为

对 `protected-managed` 列出的每个文件，cms-sync 检查三种状态：

| 状态 | 行为 |
|------|------|
| 上游包里有，下游没有 | 正常 copy（如首次安装） |
| 上下游内容相同 | unchanged |
| 上下游都存在但内容不同 | **dst 保留**，上游版本另存为 `<file>.new.<ext>`，warn 提示 |

例：`infrastructure/docker-compose.yml` 列入 `protected-managed`：
- 首次 sync → 创建 `docker-compose.yml`
- 下游改了 docker-compose.yml（如加 `include: docker-compose-pm.yml`），上游也更新了 → 下游的 docker-compose.yml 不被覆盖，上游版本写到 `docker-compose.new.yml`
- 开发者 `diff docker-compose.yml docker-compose.new.yml`，手动合并

无扩展名文件保留无扩展名（`Makefile` → `Makefile.new`）。

### 三方合并的原理

三方合并需要"共同祖先"——上次同步时的版本。脚本会读取 `.upstream.lock` 获取 `git_sha`，然后从下游项目的 git 历史中找到该版本的对应文件作为 base。

如果没有 `.upstream.lock`（首次同步）或找不到 git 历史，会回退为直接覆盖并在日志中标出 `no merge base`。

冲突时，sync 会把冲突标记（`<<<<<<< current / >>>>>>> other`）留在文件里，**不**更新 `.upstream.lock`，让你下次 sync 时仍能找到原始 base。冲突标记的 base 版本会写到 `<file>.merge-base` 方便对比。

## Glob 语法

策略文件支持简单的 glob 模式：

| 模式 | 含义 |
|------|------|
| `*` | 匹配单个路径段内的任意字符 |
| `**` | 匹配任意数量的路径段（含 `/`） |
| `?` | 匹配单个非分隔符字符 |

例：
- `contracts/**` 匹配 `contracts/`, `contracts/foo.yaml`, `contracts/sub/bar.yaml`
- `bin/*.sh` 匹配 `bin/start.sh`，但**不**匹配 `bin/sub/start.sh`

## 退出码

| 码 | 含义 |
|----|------|
| 0 | 同步成功（无冲突） |
| 1 | 同步失败 / `--check` 发现未初始化 |
| 2 | 存在合并冲突，需要手动解决 |
| 64 | 参数错误 |

## 文件清单

发布包内含：
- `bin/sync.js` — CLI 入口
- `lib/sync.js` — 同步主流程
- `lib/source.js` — tarball / URL / 目录解析
- `lib/manifest.js` — MANIFEST.json 与 .upstream.lock 解析
- `lib/parse-policy.js` — 策略文件解析
- `lib/expand-globs.js` — glob 展开
- `lib/git.js` — git merge-file 封装
- `lib/report.js` — 彩色输出
- `lib/check.js` + `lib/help.js` + `lib/cli.js` — 子命令

依赖：`tar` (npm, ~80KB)。

## 版本对应

| dist-tools 版本 | 对应上游 release 版本 |
|----------------|---------------------|
| 0.2.x | v1.x.x |
| 0.1.x | v1.0.x (初始版本, bash 实现) |

## License

UNLICENSED — Internal use only.
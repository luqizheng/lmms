# 上游同步：cms-sync 工具使用指南

> 下游业务项目（如 `project-manager`）如何把 coder-micro-services 上游发布的新版本同步进来。
> 适用：所有基于 coder-micro-services air-gap 分发包二次开发的业务项目。
> 关联：[`../user-manual/12-release-and-distribute.md`](../user-manual/12-release-and-distribute.md) §12.6 / §12.10（升级流程总览）。

---

## 1. 解决的问题

每当我们发布新的 coder-micro-services 版本，下游业务项目（基于 air-gap 包二次开发的项目，如 `project-manager`）都需要吸收上游改动。传统方式：

1. 拿到新的 `coder-micro-services-v1.x.y-airgap.tar.gz`
2. 解压
3. **手动 diff** 每个文件（配置、脚本、contracts、AGENTS.md 等）
4. 逐个判断：哪些可以覆盖？哪些我本地改过需要保留？哪些是新增？
5. git commit

这套流程**痛苦且易错**：每次升级可能要花半天对比文件，而且经常因为某个文件被漏覆盖而埋下运行时 bug。

**cms-sync 工具**把这件事自动化：
- 把每个文件按策略（managed / three-way-merge / skip）分类
- 真正改动时才动笔（diff 一致就不写）
- 双方都改过的文件用 `git merge-file` 做三方合并，自动标记冲突
- 完整记录每次同步的状态（`.upstream.lock`），下次同步能找到正确的"祖先"

---

## 2. 安装

下游项目根目录：

```bash
pnpm add -D @coder-micro-services/dist-tools
```

依赖：
- Node.js ≥ 18（开发机 / CI 基本都有）
- `git` CLI（用于三方合并；开发机默认安装）

> 包源：发布到内网 npm 私有仓库 `http://npm.kudi.repos/`，需要 `npm login` 或 `.npmrc` 指向该 registry。

CLI 入口：`cms-sync`（pnpm/npm 自动 link 到 `node_modules/.bin/`）。

---

## 3. 初始化同步策略

在下游项目根目录创建 `.upstream-sync.yaml`：

```yaml
# 上游完全管理 — sync 时直接覆盖下游副本
managed:
  - ".cargo/config.toml"
  - "pnpm-workspace.yaml"
  - "package.json"
  - "Makefile"
  - "README.md"
  - "bin/*.sh"

# 三方合并 — 双方都可能改，用 git merge-file 自动合并
# 有冲突时 sync 会在文件里留下 conflict 标记 + 写 .merge-base 副本
three-way-merge:
  - "contracts/**"
  - "scripts/**"
  - ".agents/**"
  - ".gitlab-ci.yml"
  - ".pre-commit-config.yaml"
  - "infrastructure/**"
  - "Cargo.toml"
  - "AGENTS.md"

# 跳过 — 下游独有，永远不碰
skip:
  - ".gitignore"
  - ".env"
  - ".env.example"
  - "apps/**"
  - "services/**"
  - "secrets/**"
  - ".upstream-sync.yaml"
  - ".upstream.lock"
```

策略选择原则：

| 类型 | 何时用 | 例子 |
|------|--------|------|
| `managed` | 上游版本是唯一真相源，下游无修改 | `Makefile`、`pnpm-workspace.yaml` |
| `three-way-merge` | 下游可能加业务配置、改默认值 | `contracts/error-codes.yaml`（下游可能加自定义错误码）、`infrastructure/config/api-gateway/routes/*.router.toml` |
| `skip` | 下游独有，不应被 sync 覆盖 | `.env`、`apps/`、`secrets/` |

> **首次同步建议**：先全部设为 `three-way-merge`，跑一次 dry-run 看效果。如果发现某些文件一直无人改、纯下游副本，就转 `managed`。

---

## 4. 第一次同步

### 4.1 从 v1.0.0 air-gap 包开始

```bash
# 1. 解压新版 air-gap 包到任意位置
tar -xzf coder-micro-services-v1.0.0-airgap.tar.gz -C /tmp/
# → /tmp/coder-micro-services-v1.0.0/

# 2. 在下游项目根目录，确保 .upstream-sync.yaml 已创建（§3）

# 3. 首次 sync 时 .upstream.lock 不存在（或者指向 v0.x 旧版本）
#    cms-sync 会把所有 three-way-merge 文件当作"无 base"处理 → 直接覆盖
npx cms-sync /tmp/coder-micro-services-v1.0.0/
```

**注意**：第一次 sync 没有"祖先"，三方合并退化为覆盖。如果你在第一次 sync 前已经手动从 v1.0.0 air-gap 包铺好了项目，建议先 commit 一次，再设 `.upstream.lock`：

```bash
# 在已经手动初始化好的下游项目根目录
echo '{
  "version": "v1.0.0",
  "git_sha": "<v1.0.0 release commit 的 sha>",
  "synced_at": "<ISO timestamp>"
}' > .upstream.lock
git add .upstream.lock && git commit -m "chore: bootstrap sync state to v1.0.0"
```

这样后续 v1.0.x → v1.1.0 的升级就有正确的 base 了。

### 4.2 验证

```bash
# 看当前 sync 状态
npx cms-sync --check
```

输出：
```
[cms-sync] current sync state
[cms-sync]   version: v1.0.0
[cms-sync]   git_sha: a1b2c3d...
[cms-sync]   synced_at: 2026-07-31T10:00:00Z
[cms-sync] policy file: /path/to/project/.upstream-sync.yaml
```

---

## 5. 升级到新版本

### 5.1 标准流程

```bash
# 拿到新版本 air-gap 包（例 v1.1.0）

# 第 1 步：干跑 — 看会发生什么，不修改文件
npx cms-sync --dry-run ./coder-micro-services-v1.1.0-airgap.tar.gz
```

输出示例：
```
[cms-sync] new version: v1.1.0  git_sha: newabc1
[cms-sync] previous version: v1.0.0  git_sha: oldxyz0

[cms-sync] managed (auto-overwrite)
  updated   Makefile
  updated   bin/start.sh

[cms-sync] three-way merge
  merged    contracts/error-codes.yaml
  new file  infrastructure/config/iam/extra-routes.toml

[cms-sync] summary
  managed overwritten: 2
  three-way merged: 1
  new files added: 1
  unchanged: 5
```

**审阅输出**，确认：
- 哪些文件被覆盖（managed）—— 没问题就继续
- 哪些三方合并成功 —— 没问题就继续
- 是否有冲突（CONFLICT 行）—— **必须手动解决后才能跑实际 sync**
- 新增的文件 —— 确认是你要接收的

```bash
# 第 2 步：实际同步
npx cms-sync ./coder-micro-services-v1.1.0-airgap.tar.gz

# 第 3 步：检查 git 状态
git status
git diff contracts/error-codes.yaml   # 看三方合并结果

# 第 4 步：跑 CI 门禁，确保新版本兼容
pnpm check
pnpm test
node scripts/check-service-standard.js
node scripts/contracts-gen.js --check

# 第 5 步：提交
git add -A
git commit -m "chore(sync): upstream v1.0.0 → v1.1.0"
```

### 5.2 从 URL 拉

cms-sync 支持直接拉 HTTP/HTTPS：

```bash
npx cms-sync https://internal-release.example.com/v1.1.0.tar.gz
```

适合 CI 自动同步（无需先把 tarball 下到本地）。

### 5.3 在线升级（已有 docker registry 访问）

如果你的环境已经能 `docker pull` 新镜像（不需要 air-gap tarball），传统 `make upgrade TAG=v1.1.0` 仍然适用 —— 那只是换镜像，不涉及配置文件变更。

cms-sync 主要解决的是**配置文件 / 脚本 / contracts**层面的升级，**不**替代镜像升级。

---

## 6. 处理冲突

三方合并时，如果上游和下游改了同一文件的同一区域，cms-sync 会在文件里留下 git 标准的冲突标记：

```yaml
errors:
  - code: 1001
    message: "user not found"
<<<<<<< current
  - code: 9999
    message: "project-specific-error"
=======
  - code: 1002
    message: "new upstream error"
>>>>>>> other
```

同时：
- `.upstream.lock` **不**更新（保留旧 sha，下次 sync 还能找到原始 base）
- 冲突文件的 base 版本写到 `<file>.merge-base` 方便对比

**退出码 = 2**，CI 可以用这个码阻断。

### 6.1 解决冲突

```bash
# 1. 用编辑器打开冲突文件
$EDITOR contracts/error-codes.yaml

# 2. 决定保留哪些 — 通常两种模式：
#    a) 全保留：9999 (project-specific) + 1002 (upstream new)
#    b) 命名空间隔离：把项目自定义错误码放独立文件
#       contracts/error-codes.yaml ← 仅 upstream
#       contracts/project-errors.yaml ← 仅 project-specific (跳过 sync)

# 3. 删 conflict 标记 + 保存

# 4. 删除 .merge-base 副本（可选，sync 不会自动清理）
rm contracts/error-codes.yaml.merge-base

# 5. 验证 sync 状态：现在 .upstream.lock 还是旧的，需要手动跑 sync
#    或 commit 后下一次 sync 自动前进
```

### 6.2 重跑 sync 验证冲突已解决

```bash
# 修改完后再跑一次 dry-run — 应该不再有 CONFLICT
npx cms-sync --dry-run ./coder-micro-services-v1.1.0-airgap.tar.gz
```

如果还有 CONFLICT，说明还有未解决的冲突标记。

### 6.3 取消 sync 回滚

如果 sync 中途发现搞砸了（实际 sync 还没跑成功时退出码非 0 是写盘的；中途崩溃则可能有部分文件被改）：

```bash
# 取消 sync 期间的写盘
git checkout -- .
git clean -fd  # 删 sync 新增的文件（注意：会删你不想删的未 tracked 内容）
```

更安全的做法：**跑 sync 前先 commit 当前状态**：

```bash
git add -A && git commit -m "checkpoint: before upstream sync"
npx cms-sync ./coder-micro-services-v1.1.0-airgap.tar.gz
# 如果出问题：
git reset --hard HEAD
```

---

## 7. Glob 语法

`.upstream-sync.yaml` 用简单的 glob 匹配 dist 包里的文件：

| 模式 | 含义 |
|------|------|
| `*` | 匹配单个路径段内的任意字符（不含 `/`） |
| `**` | 匹配任意数量的路径段（含 `/`） |
| `?` | 匹配单个非分隔符字符 |

例：

```yaml
managed:
  - "bin/*.sh"           # 匹配 bin/start.sh、bin/stop.sh
                         # 不匹配 bin/sub/start.sh（跨段）

  - "bin/**/*.sh"        # 匹配 bin/ 下任意深度的 .sh 文件

three-way-merge:
  - "contracts/**"       # 匹配 contracts/ 下所有文件（含子目录）
  - "infrastructure/**"  # 匹配整个 infrastructure 目录树
```

> 模式始终按 POSIX 路径解析（`/`），Windows 上同样工作。

---

## 8. 退出码

| 码 | 含义 | CI 行为 |
|----|------|---------|
| 0 | 同步成功（无冲突） | 视为通过 |
| 1 | 同步失败 / `--check` 发现未初始化 | 视为失败 |
| 2 | 存在合并冲突 | 视为失败，需人工解决 |
| 64 | 参数错误 | 视为失败 |

CI 示例：

```yaml
# .gitlab-ci.yml
sync-upstream:
  script:
    - npx cms-sync ./dist.tgz
  allow_failure:
    exit_codes:
      - 2   # 冲突允许保留，让 MR 阻塞
```

---

## 9. 故障排查

| 现象 | 原因 | 解决 |
|------|------|------|
| `no .upstream-sync.yaml found` | 没创建策略文件 | 按 §3 创建 |
| `no previous sync recorded — 3-way merge will fall back to no-base` | 首次 sync，或 lock 文件不在 | 首次属正常；若非首次，检查 `.upstream.lock` 是否被 `.gitignore` |
| `Cannot find module 'tar'` | pnpm/npm 没装依赖 | `pnpm install` |
| `git: command not found` | Windows 没装 Git | 装 [Git for Windows](https://gitforwindows.org/) |
| `git merge-file` 退出但没合并 | 上游和下游的 base 不一致（极少见） | 检查 `.upstream.lock` 是否指向真实存在的 commit |
| sync 后大量 CONFLICT | base 选错了（lock 里 git_sha 不是真实 commit） | 把 `.upstream.lock` 的 git_sha 改成"上次成功 sync 后下游 commit" 的 sha |
| 想跳过某次 sync 回到上一版本 | sync 是单向覆盖 | 走 git：`git checkout HEAD~1 -- .` 然后改 `.upstream.lock` 的 git_sha |
| 想批量接受上游改动（不冲突） | 期望默认行为就是这样 | 默认就只覆盖 diff 的文件；无 diff 的不写 |

---

## 10. 与现有升级流程的关系

cms-sync **不替代**已有的升级方式，是补充：

| 升级方式 | 适用 | 命令 |
|----------|------|------|
| **cms-sync** | air-gap 私有化部署，业务项目需要同步配置/脚本/contracts 改动 | `npx cms-sync ./dist.tgz` |
| **make upgrade** | 镜像版本升级（不动配置） | `make upgrade TAG=v1.1.0` |
| **pnpm update @coder-micro-services/dist-tools** | 仅更新同步工具本身 | `pnpm update @coder-micro-services/dist-tools` |

典型升级组合：

```bash
# 1. 同步工具升级（如果新版本）
pnpm update @coder-micro-services/dist-tools

# 2. 同步上游配置文件 / 脚本 / contracts
npx cms-sync ./coder-micro-services-v1.1.0-airgap.tar.gz
# 解决冲突（如有）

# 3. 升级 Docker 镜像
make upgrade TAG=v1.1.0

# 4. 跑全量 CI 门禁
pnpm check && pnpm test
node scripts/check-service-standard.js
node scripts/contracts-gen.js --check

# 5. 提交
git add -A && git commit -m "chore(sync): upstream v1.0.0 → v1.1.0"
```

---

## 11. 相关文档

- [`../user-manual/12-release-and-distribute.md`](../user-manual/12-release-and-distribute.md) — air-gap 包结构、镜像发布、12 章发布全流程
- [`./docker-registry-config`](./docker-registry-config) — Docker / Cargo / Maven / npm 包源配置
- [`./commit-convention`](./commit-convention) — 提交规范（含 chore(sync) 前缀）
- `release/dist-tools/README.md`（包内） — cms-sync 工具技术参考
- [npm 私有仓库 `http://npm.kudi.repos/`](http://npm.kudi.repos/) — `@coder-micro-services/dist-tools` 包主页
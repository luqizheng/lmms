# 发布 CLI（cms-release）设计

日期：2026-10-29
状态：已定稿（随本 commit 实现）
范围：`release/release-cli/`（新增）+ `release/dist-template`（升级链路修复）+ 根 `package.json` / `README.md`

## 1. 背景与问题

上游仓的发布能力已存在但**散落且无统一入口**：

| 能力 | 现有实现 | 问题 |
|------|----------|------|
| 构建并推送镜像 | `scripts/build-docker.js` | 参数多（-v / --push / --load-and-push / --platform），无默认版本推断 |
| 打离线包 | `release/dist-template/scripts/release.js` | 与 build-docker 的版本号要人工保持一致 |
| 版本账本 | git tag（v1.5.1）vs `dist-template/Makefile IMAGE_TAG ?= 1.0.0` vs CHANGELOG 顶条 v1.0.0 | 三处散点，无命令可查/收口 |
| 下游升级 | `dist-template/bin/upgrade.js <tag>` | **空操作缺陷**（见 §3） |

## 2. 目标

1. 一个零依赖 JS CLI `cms-release`（npm 包 `@digua/release-cli`，与 `@digua/dist-tools`/`cms-sync` 同族），统一：查版本（status）、发镜像（publish）、打包（package）、升版本（bump）、下游升级（upgrade）。
2. 修复下游升级链路：`make upgrade TAG=x` / `cms-release upgrade` 真正切换镜像版本。
3. 版本规范不变：git tag 带 `v`（v1.5.1），镜像 tag / 包名不带 v（1.5.1）。CLI 入口两种都收，内部归一化。

## 3. 升级链路缺陷与修复

现状：`transform-compose.js` 把镜像 tag **字面量写死**进分发包 compose（`image: .../api-gateway:1.5.0`）；`bin/upgrade.js` 的 `NEW_TAG` 只用于 echo，`docker compose pull` 永远拉包内写死 tag → 升级是空操作。

修复（三处联动）：

1. `transform-compose.js`：image 行改为 `${IMAGE_TAG:-<version>}`（compose 插值带默认值，包内默认 = 打包时版本，行为向后兼容）。
2. `release.js`：打包时同步改写 dist 内 `.env.example` 的 `IMAGE_TAG=<VERSION>`（否则旧 .env 的 1.0.0 会盖掉新默认值）与 dist 内 `Makefile` 的 `IMAGE_TAG ?=`/banner。
3. `bin/upgrade.js`：接受 `v1.6.0` / `1.6.0`，归一化为 no-v；先改写 `.env` 的 `IMAGE_TAG=` 行（持久化），再以 `IMAGE_TAG=<tag>` 进程环境变量（优先级高于 .env）执行 `docker compose pull && up -d`。

Compose 插值优先级：进程环境变量 > `--env-file`，这是修复的机制基础。

## 4. CLI 设计

```
cms-release status                                  # git tag / 分支 / 脏树 / Makefile / CHANGELOG 散点对账
cms-release publish [services...] [-v 1.5.1] [--push|--load-and-push]
                    [--platform linux/amd64] [--dry-run]
cms-release package [-v 1.5.1] [--no-images] [--dry-run]
cms-release bump <1.6.0|--patch|--minor|--major> [--tag] [--force] [--dry-run]
cms-release upgrade <dist-dir> <1.6.0>              # 委托 dist 的 bin/upgrade.js
```

- **委托不重写**：publish/package 直接 spawn 既有 `build-docker.js` / `release.js`（`stdio: inherit`），逻辑单点；CLI 只做版本推断、参数归一、前置检查。
- **版本推断**：`-v` 缺省时取「最新 git v-tag 去 v」；仓外（无 tag）报错要求显式 `-v`。
- **bump**：只改 `release/dist-template/Makefile`（`IMAGE_TAG ?=` + help banner 两处，纯函数可测）。可选 `--tag` 打 annotated git tag（本地可逆）。`--force` 绕过「版本回退」与「脏树」守卫。CHANGELOG 内容不自动生成，仅在顶条版本 ≠ 目标时提示。
- **`--dry-run`**：publish/package/bump 打印将执行的 argv/将写入的 diff，不落盘不 spawn。
- **root 定位**：从 cwd 向上找含 `scripts/build-docker.js` 的目录（仓内 `pnpm release:cli` 与全局安装后仓内执行均可）；status/bump 可用 `--root` 显式指定。
- 退出码沿用 dist-tools 约定：0 / 1（错误）/ 64（用法）。
- CJS + Node ≥18 + 零 dependencies（ANSI 自绘，依赖仅 node: 内置），测试用 `node --test`（与 dist-tools 同风格）。

## 5. 非目标

- 不自动生成 CHANGELOG 条目内容；不自动 push tag / 镜像（push 仍需显式 --push）。
- 不动 Argo Rollouts 灰度（TODO 中独立项）；不做多包（dist-tools）联动发布。
- 不改 `build-docker.js` / `release.js` 本身的行为。
- 下游在部署机上的日常 up/down/logs 仍走 Makefile（已够用），CLI 只补「升级」这一环。

## 6. 测试与验收

- 单测（node --test，全纯函数）：version 归一化/比较/自增/最新 tag 选取；Makefile 读改（幂等、保格式）；argv 解析与 exit code。
- 既有门禁不回归：`pnpm run release:test`（release-transforms）照旧通过。
- 冒烟：`--help`、`status`、`bump --dry-run`、`publish --dry-run` 在仓内可跑通。
- 升级链路修复的实机验证（compose pull 真拉新 tag）属部署机操作，不在本 commit 验证范围，README 注明。

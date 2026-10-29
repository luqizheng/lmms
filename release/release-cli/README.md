# @digua/release-cli

`cms-release` — coder-micro-services 发布 CLI：统一**发布镜像 / 打离线包 / 版本收口 / 下游升级**入口。与 `@digua/dist-tools`（`cms-sync`）同族，零 dependencies，Node ≥ 18，跨 Linux/macOS/Windows。

## 安装与运行

```bash
# 框架仓内（免安装）
pnpm run release:cli -- status

# 全局安装（内网 npm registry）
npm i -g @digua/release-cli
cms-release status
```

## 命令

| 命令 | 作用 | 委托到 |
|------|------|--------|
| `status` | git tag / Makefile / CHANGELOG 版本对账 | — |
| `publish [services...]` | 构建并（可选）推送服务镜像 | `scripts/build-docker.js` |
| `package` | 打离线分发包（airgap tar） | `release/dist-template/scripts/release.js` |
| `bump <ver\|--patch\|--minor\|--major>` | 版本收口：改写 dist-template Makefile，可选 `--tag` 打 git tag | — |
| `upgrade <dist-dir> <ver>` | 在解压后的分发包目录就地升级 | 分发包 `bin/upgrade.js` |

`-v` 缺省时取「最新 git tag 去 v」。`--dry-run` 打印将执行的命令 / 将写入的 diff。完整选项见 `cms-release --help`。

## 版本规范

- git tag 带 `v`（`v1.5.2`），镜像 tag / 包名不带 `v`（`1.5.2`）；CLI 入口两种都收，内部归一化。
- 发布节奏：`bump --tag` → `publish -v x.y.z --load-and-push` → CHANGELOG 补条目 → `package -v x.y.z` → push tag。

## 下游升级机制（重要）

分发包 compose 的镜像 tag 由打包时写成 `${IMAGE_TAG:-<打包版本>}`。compose 插值优先级为
**进程环境变量 > `--env-file`**，因此升级链路是：

1. `bin/upgrade.js <ver>` 先把 `.env` 的 `IMAGE_TAG=` 行改写为新版本（持久化）；
2. 以 `IMAGE_TAG=<ver>` 环境变量执行 `docker compose pull && up -d`（volumes 保留）。

也可直接 `make upgrade TAG=v1.5.3`（Makefile 目标转发到同一脚本）。

## 测试

```bash
pnpm run release:cli:test
```

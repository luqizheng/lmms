# Public Release Pipeline

正式公开发行由 GitHub Actions 执行，触发方式有两种：推送正式版本 tag，或对手动指定一个**已存在**的 tag 补发。

## 1. 触发条件

### 1.1 正常发版（新建 tag）

```bash
git checkout main
git pull --ff-only origin main
git tag vX.Y.Z
git push origin vX.Y.Z
```

### 1.2 补发（tag 已存在，不新建、不移动）

GitHub → Actions → `Public Release` → `Run workflow`，填写 `tag`（必须严格匹配 `vMAJOR.MINOR.PATCH`）。

两种触发方式执行**完全相同**的验证：tag 格式校验 + tag 对应 commit 必须可从 `origin/main` 到达 + 全套门禁与 dry-run 预检。任一失败都会阻止三个发布 job。不要覆盖或强制移动已发布 tag。

## 2. 发布目标

| 生态 | 发布内容 | 目标 |
|---|---|---|
| Rust | `libs/*`、`sdk/rust/*` | crates.io |
| TypeScript | `@digua/contracts`、`@digua/http`、`@digua/{iam,audit,file,notification,prue}-services` | npmjs.org |
| Docker | 平台服务镜像 | Docker Hub |

Java/Maven 不属于当前公开发布流程。所有公开 npm 包统一使用 `@digua/` scope（未加 scope 的 `file-services` / `notification-services` 在 npmjs.org 已被他人占用）。

公开发布的包必须声明真实开源许可证（`Apache-2.0`），**不得**使用 `UNLICENSED`：`UNLICENSED` 等价于“未授予任何权利”，与公开分发目标冲突。


## 3. GitHub Secrets

| Secret | 用途 |
|---|---|
| `CRATES` | crates.io API token |
| `NPMPUBLISHKEY` | npmjs.org automation token |
| `DOCKER_USER_NAME` | Docker Hub 用户名/namespace |
| `DOCKER_KEY` | Docker Hub access token |

凭据只注入对应的发布 job。PR、普通分支 push 和验证 job 不读取发布凭据。

## 4. 发布前门禁

发布 job 必须等待 validate job 完成。验证包括 Rust fmt/Clippy/单元测试、仓库规范与契约门禁、OpenAPI 漂移、CLI 测试和 npm 包构建。任何验证失败都会阻止三个发布 job。

## 5. 版本规则

- Docker 镜像版本来自根 `Cargo.toml` 的 workspace service version；tag 去掉前缀 `v`。
- Rust crates 和 npm packages 使用各自 manifest 的独立版本号。
- Docker Hub 镜像只推送不可变版本 tag，不更新 `latest`。
- Docker 镜像发布成功后，workflow 上传生成的两个 `.image.env` 文件作为 artifact；不会自动提交到 main 或部署生产。

## 6. 首次公开发布前必须确认

- crates.io 中全部 crate 名称归本项目所有（`cargo owner --list`），且待发版本尚未被占用。
- npmjs.org 中的 `@digua` scope（organization）归本项目所有，且 token 具备发布权限。
- Docker Hub namespace 与 `DOCKER_USER_NAME` 一致，且 token 具备 push 权限。
- 四个 GitHub Secrets 均有效，且权限只覆盖对应发布目标。
- 所有公开发布包声明 `Apache-2.0` 且仓库根存在 `LICENSE`。

## 7. 失败处理与断点续发

三个生态是独立 job，可单独重跑；补发时用 `workflow_dispatch` 指定同一个 tag，不要新建 tag。

- Rust：目标版本已在 registry（`already exists`）→ 跳过并继续，支持半途失败后重跑。
- npm：目标版本已存在（`previously published` / `EPUBLISHCONFLICT`）→ 跳过并继续。
- Docker：同一不可变版本 tag 重推会覆盖同名 tag，重建是幂等的；重跑前先确认上一次 job 已完成的镜像范围，避免与并行发布冲突（workflow 已用 `public-release-tags` concurrency 组串行化）。


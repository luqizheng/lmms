# Public Release Pipeline

正式公开发行由 GitHub Actions 执行，触发方式为推送正式版本 tag。

## 1. 触发条件

```bash
git checkout main
git pull --ff-only origin main
git tag vX.Y.Z
git push origin vX.Y.Z
```

只有严格匹配 `vMAJOR.MINOR.PATCH` 且 commit 可从 `main` 到达的 tag 才能进入发布阶段。不要覆盖或强制移动已发布 tag。

## 2. 发布目标

| 生态 | 发布内容 | 目标 |
|---|---|---|
| Rust | `libs/*`、`sdk/rust/*` | crates.io |
| TypeScript | contracts、http、五个服务 SDK | npmjs.org |
| Docker | 平台服务镜像 | Docker Hub |

Java/Maven 不属于当前公开发布流程。

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

- crates.io 中全部 crate 名称可用，且各版本尚未被其他人占用。
- npmjs.org 中所有包名可发布；尤其检查当前未加 scope 的服务 SDK 包名是否已被占用，以及账号是否拥有发布权限。
- Docker Hub namespace 下的服务 repositories 已创建或允许首次 push。
- 四个 GitHub Secrets 均有效，且权限只覆盖对应发布目标。

## 7. 失败处理

三个生态是独立 job，可单独重跑。Rust crates 与 npm 同版本重复发布按 registry 已存在错误识别并跳过；其他错误应修复后再重试。不要为了重试而移动已经发布的 tag。Docker 镜像使用同一版本 tag 重推前，先确认上一次 job 是否已完成部分服务推送。

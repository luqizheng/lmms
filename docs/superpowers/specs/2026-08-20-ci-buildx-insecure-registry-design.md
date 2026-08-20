# ci buildx insecure-registry for zhcoder-docker-registry

- Date: 2026-08-20
- Branch: `fix/ci-buildx-insecure-registry`
- Triggered by: pipeline 23541 (`build_images:pre-staging`) failing with
  `failed to do request: Head "https://zhcoder-docker-registry.com:8000/v2/...": http: server gave HTTP response to HTTPS client`
  on `FROM ${BASE_REGISTRY}/mysql:8.0`.

## Problem

`build_images:pre-staging` 新增了 `docker buildx build --push` 拉
`infrastructure/mysql-replica/Dockerfile`（f4a7f7f 引入、6a87e6e 修 YAML），
base image 走 `zhcoder-docker-registry.com:8000/official/mysql:8.0`。该 registry
仅 HTTP（:8000 端口明文，无 TLS），buildkit 默认走 HTTPS，导致 base image
metadata HEAD 请求被服务端拒：

```
#2 [internal] load metadata for zhcoder-docker-registry.com:8000/official/mysql:8.0
#2 ERROR: failed to do request: Head "https://zhcoder-docker-registry.com:8000/v2/official/mysql/manifests/8.0": http: server gave HTTP response to HTTPS client
```

`build_images:staging` / `build_images:tagged` 走 `scripts/build-docker.js`
同样依赖同一个内网 base registry，理论上一旦该 registry 上挂任何带 build
步骤的服务（`prue-admin-backend` / `frontend` 的多阶段构建、`mysql-replica`），
buildkit 都会在 metadata 阶段撞墙；只是历史 job 命中的 Dockerfile 都是无 base
依赖的纯 `FROM scratch`/空 base，没有暴露。

## Root cause

`.gitlab-ci.yml` 三处（`build_images:pre-staging` / `staging` / `tagged`）
的 `docker buildx create --name xbuilder --use` 隐含 `docker-container` driver，
buildkit 容器内的 registry 协议决策**独立于 host daemon 的
`/etc/docker/daemon.json`**。即便 host runner 的 daemon 加
`insecure-registries` 也救不了 buildkit 容器内的请求。

`before_script` 里 `docker login zhcoder-docker-registry.com:8000 ... 2>&1`
只是把凭证写进 `~/.docker/config.json`，不改变协议 → 这就是 login 一直被
`>/dev/null` 吞掉错误也"看上去成功"的根因。

## Fix

让 buildkit 容器**自己**认识这个 registry 是 HTTP。每 job 内写一份
buildkitd 配置 + 用 `--config` 重建 xbuilder：

```bash
# before_script
cat > /tmp/buildkitd.toml <<'EOF'
[registry."zhcoder-docker-registry.com:8000"]
  http = true
EOF

# script（替换原 `--use 2>/dev/null || ... use xbuilder` 那行）
sudo docker buildx rm xbuilder 2>/dev/null || true
sudo docker buildx create \
  --name xbuilder \
  --driver docker-container \
  --config /tmp/buildkitd.toml \
  --use
sudo docker buildx inspect --bootstrap
```

三处 jobs 同样处理：
- `build_images:pre-staging`（最先暴露在 pipeline 23541）
- `build_images:staging`（同 runner / 同 xbuilder，预防）
- `build_images:tagged`（同 runner / 同 xbuilder，预防）

## Why this shape

- **buildkitd `http = true` 是正解**：buildkit 配置项明确支持；不依赖
  host runner 改 `/etc/docker/daemon.json`（CI 共享 runner，重启 docker
  会 kill 其他 job 的容器，副作用不可接受）。
- **`docker buildx rm` 先于 create**：避免上一次 runner 用过的 xbuilder
  残留旧 config（--config 只在 create 时生效，update 子命令不存在）。
- **`/tmp/buildkitd.toml`**：shell executor runner 是按 job 创建临时 rootfs，
  路径稳定且自带清理。
- **不动 `docker login`**：登录失败被吞是无害的（buildkit 通过
  `--config` 直接配置协议，credentials 由 `~/.docker/config.json`
  mount 进 buildkit 容器），改它属于另一个 PR 范畴。
- **三处 jobs 同步改**：同一个 xbuilder 名字被这三个 job 共享（main 上
  只跑其中之一，但 staging/tagged 也可能并发/串行触发），任何一个忘改
  都会让 buildkit 容器回退到默认 config，下次 build 又中招。

## Verification

- 本地 `cat /tmp/buildkitd.toml` 验证三处都写入同一份内容。
- `grep -c '\-\-config /tmp/buildkitd.toml' .gitlab-ci.yml` 应得 3（三 job）。
- `grep -c 'http = true' .gitlab-ci.yml` 应得 3（写 buildkitd.toml 的 heredoc）。
- 合并到 main 后，`build_images:pre-staging` 在 main pipeline 跑通
  （mysql-replica build 推 mirror 完成）即视为修复有效。

## Risk

低。buildkitd config 只影响 buildkit 容器内的 registry 协议决策，
对其他 build 步骤无副作用。三处 jobs 加的 5 行（每个 job）改动彼此
同构，回归面窄。

## Related

- 前置 spec：[`2026-08-19-pre-staging-mysql-replica-registry-fix-design.md`](2026-08-19-pre-staging-mysql-replica-registry-fix-design.md)
  — 把 mysql-replica 纳入 commit-pinned registry 部署模型（f4a7f7f / 6a87e6e）。
  本 spec 是它的下一步：registry 模型就绪后，buildkit 需要知道怎么访问它。
- CI 历史 pipeline：23381（加 buildx call）→ 23487（修 YAML 转义）→ **23541（本修复目标）**。

## Out of scope

- 不改 host runner 的 `/etc/docker/daemon.json`：CI shell runner 多 job 共享，
  重启 daemon 影响面太大；本次 fix 仅让 buildkit 自洽。
- 不改 `docker login` 行：与本修复正交，留待另一个 cleanup PR。
- 不动 `scripts/build-docker.js`：`docker buildx build` 默认走当前 builder
  （已被 `docker buildx use xbuilder` 切到），buildkit 配置由 create 时注入。
# pre-staging mysql-replica registry fix

- Date: 2026-08-19
- Branch: `fix/pre-staging-mysql-replica-registry`
- Triggered by: deploy job 23381 (`deploy:pre-staging`) failing with
  `failed to resolve reference "docker.io/coder/mysql-replica:8.0": not found`.

## Problem

`infrastructure/docker-compose-infra.yml` 的 `mysql-replica` service 声明了
`build:` + `image: coder/mysql-replica:8.0`（后者作为本地 tag）。本地 dev
走 `docker compose build` 没问题。

CI pre-staging 流程：

1. `build_images:pre-staging` 推 9 个 Rust 服务 + frontend 到
   `zhcoder-docker-registry.com:8000/micro-server/<svc>:<sha>`，
   **不包含 mysql-replica**。
2. `release:pre-staging` 调 `transform-compose.js` 把
   `docker-compose-services.yml` 的 `build:` 块换成 registry image，
   **`docker-compose-infra.yml` 不在 transform 列表里**。
3. `deploy:pre-staging` SSH 到 staging 跑 `deploy-staging.sh`，
   STEP 1 是 `docker compose pull`。compose 看到 `mysql-replica` 的
   `image: coder/mysql-replica:8.0`（无 registry 前缀）→ 默认从
   docker.io 拉 → `not found`。

## Fix

把 `mysql-replica` 纳入现有的 commit-pinned registry 部署模型，与
其他 9 个服务 + frontend 完全对齐。改动 5 个文件：

1. **`infrastructure/mysql-replica/Dockerfile`**：加
   `ARG BASE_REGISTRY=zhcoder-docker-registry.com:8000/official`，
   `FROM ${BASE_REGISTRY}/mysql:8.0`，与 `services/*/Dockerfile` 一致。
2. **`release/dist-template/scripts/transform-compose.js`**：
   - `IMAGE_MAP` 加 `mysql-replica → micro-server/mysql-replica`。
   - 新增 `skipCoexistingImageTag()` helper：处理 `build:` + `image:`
     共存场景（mysql-replica 唯一触发），否则 transform 后 compose 拿到
     两个 `image:` 报错。
3. **`scripts/release-staging.js`**：transform 列表追加
   `docker-compose-infra.yml`。其余 5 个 infra 服务（mysql / redis /
   consul / rabbitmq / registrator）只声明 `image:`，`IMAGE_MAP` 不含
   它们，transform-compose 原样保留，无副作用。
4. **`.gitlab-ci.yml` `build_images:pre-staging`**：在
   `node scripts/build-docker.js` 后追加 `docker buildx build --push`
   `infrastructure/mysql-replica/`，产出
   `zhcoder-docker-registry.com:8000/micro-server/mysql-replica:${CI_COMMIT_SHORT_SHA}`。
5. **`infrastructure/README.md`**：mysql-replica 描述补全本地 build vs
   CI registry 两条路径。

## Deploy topology (after fix)

```
build_images:pre-staging
   ├─ node scripts/build-docker.js --push   # 9 Rust 服务 + frontend
   └─ docker buildx build --push            # mysql-replica（新增）
        ↓
release:pre-staging
   ├─ transform docker-compose-services.yml # build → image: <registry>/micro-server/<svc>:<sha>
   └─ transform docker-compose-infra.yml    # 新增；mysql-replica 同上
        ↓
deploy:pre-staging → deploy-staging.sh
   └─ docker compose pull (含 mysql-replica，从 registry 拉)  ← 不再 fail
```

## Why this shape

- **不用 build-on-staging**：保留 commit-pinned 不可变镜像 + registry
  拉取语义，与其他服务一致；staging 服务器无需装 docker buildx
  也不持 Dockerfile 缓存。
- **不用 `${BASE_REGISTRY}` 前缀**：mysql-replica 不是 docker.io 的
  公开镜像（`coder/*` 不是 Docker Hub org），它必须先经过 CI build
  推到 micro-server repo，所以 transform 直接写
  `<registry>/micro-server/mysql-replica:<sha>`，不是
  `${BASE_REGISTRY}coder/mysql-replica:8.0`。

## Verification

- ✅ 本地 `node scripts/release-staging.js --commit aeb9569 --output /tmp/test`
  跑通；输出的 `docker-compose-infra.yml` 里 mysql-replica 块正确替换
  为 `image: zhcoder-docker-registry.com:8000/micro-server/mysql-replica:aeb9569`，
  其余 5 个 infra service 原样保留。
- ✅ `docker compose config` 解析 23 个 service 全过。
- ✅ `node scripts/check-service-standard.js` 服务规范检查通过。
- ✅ `node scripts/check-spec.js` 契约规范检查通过（warning 均为
  pre-existing 缺 summary，与本修复无关）。
- ✅ `node scripts/contracts-gen.js --check` 契约无 drift。
- ✅ `node scripts/check-db-naming.js` 表名服务前缀检查通过。
- ✅ `node scripts/dump-openapi-specs.js` + `git diff --exit-code
  services/*/openapi.yaml` 无 drift。

## Out of scope

- 不动 `release/dist-template/release/coder-micro-services-1.0.{3,4}/` 历史
  离线分发产物（已在历史 tag 里，不影响新部署）。
- 不动其他 5 个 infra service（它们已经是 `${BASE_REGISTRY}<image>`
  写法，pull 正常工作）。
- 不改 backend services 镜像构建路径（`scripts/build-docker.js`
  + `transform-compose.js` 处理 services/ 下业务服务）。

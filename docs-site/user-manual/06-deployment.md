# 06. 部署

> 镜像怎么出、CI 怎么走、Promote 到生产。

---

## 6.1 构建镜像

`scripts/build-docker.js` 支持交互式 / 参数式两种模式，可打版本号 tag、多 CPU 架构、可选推送到内网 registry（`zhcoder-docker-registry.com:8000/micro-server/<service>`）。

```bash
# 交互式（无参数）：依次问 版本号 / 架构 / 是否推送 / 服务子集，默认推送 Y
pnpm run docker:build

# 参数式：全部服务，版本号 1.0.0，直接推送
pnpm run docker:build -- -v 1.0.0 --push

# 本地留镜像 + 推送（单架构 --load 后再 docker push 每 tag）
pnpm run docker:build -- --load-and-push -v 1.0.0

# 多架构（buildx 多架构无法 --load，自动开启 --push）
pnpm run docker:build -- --platform linux/amd64,linux/arm64 --push

# 单服务
pnpm run docker:build:iam-service          # = node scripts/build-docker.js iam-service
```

| 选项 | 作用 |
|---|---|
| `-v, --version <ver>` | 版本号 tag（如 `1.0.0`），同时打 `:<ver>` 与 `:latest` |
| `--push` | `buildx --push` 直接推送，不留本地镜像 |
| `--load-and-push` | 单架构：`--load` 后再 `docker push` 每 tag（本地留镜像 + 推送）；多架构：回退 `--push` |
| `--no-push` | 不推送（参数式默认） |
| `-p, --platform <list>` | CPU 架构，逗号分隔（默认 `linux/amd64`），如 `linux/amd64,linux/arm64` |
| `-i, --interactive` | 强制交互式 |

> **本地 vs CI**：本地参数式默认**不推送**（需 `--push`/`--load-and-push`）；交互式默认推送 Y。CI 走 `.gitlab-ci.yml` 的 `build_*` job 自动 `docker buildx build` + `docker push`，打 `:<commit-sha>` + `:latest`。`node scripts/build-docker.js --help` 看全部选项。

> **多架构注意**：arm64 构建需宿主启用 qemu binfmt：`docker run --privileged --rm tonistiigi/binfmt --install all`。

---

## 6.2 CI 流水线（GitLab）

三阶段：`test` → `build` → `deploy`。

### test 阶段

| Job | 命令 |
|-----|------|
| `service_standard_check` | `node scripts/check-service-standard.js` |
| `spec_check` | `node scripts/check-spec.js` |
| `contracts_check` | `node scripts/contracts-gen.js --check` |
| `spec_generate` | `scripts/dump-openapi-specs.sh`（产出 yaml 给 drift 检测） |
| `rust_test` | `cargo test --workspace --lib`（容器内；集成测由本地 `scripts/run-tests-docker.js` 覆盖） |
| `e2e_api_test` | `pnpm -C tests/e2e test:api` |
| `e2e_ui_test` | `pnpm -C tests/e2e test:ui` |

任何一项失败 → 阻断后续阶段。

### build 阶段

镜像打 tag `CI_COMMIT_SHORT_SHA` 与 `latest`，推送到内网 registry。

### deploy 阶段

SSH 到部署机 → 拉新镜像 → 重启 compose。

---

## 6.3 CI 门禁速查

| 门禁 | 脚本 |
|------|------|
| 服务标准 | `node scripts/check-service-standard.js` |
| Spec 校验 | `node scripts/check-spec.js` |
| 契约漂移 | `node scripts/contracts-gen.js --check` |
| OpenAPI 漂移 | `scripts/dump-openapi-specs.sh` + `git diff --exit-code` |
| Rust 测试 | `node scripts/run-tests-docker.js <svc>` |
| E2E API | `pnpm -C tests/e2e test:api` |
| E2E UI | `pnpm -C tests/e2e test:ui` |

**push 前本地一键过一遍**：

```bash
node scripts/check-service-standard.js && \
  node scripts/check-spec.js && \
  node scripts/contracts-gen.js --check && \
  scripts/dump-openapi-specs.sh && \
  git diff --exit-code -- 'services/*/openapi.yaml'
```

> 也可装 pre-commit（`.pre-commit-config.yaml`）做本地拦截：`pre-commit install`。

---

## 6.4 手动部署（紧急 / 演练）

```bash
ssh kudi@192.168.2.128
cd /opt/coder-micro-services
docker-compose -f infrastructure/docker-compose.yml down
docker-compose -f infrastructure/docker-compose.yml pull
docker-compose -f infrastructure/docker-compose.yml up -d
```

---

## 6.5 发布节奏

- **常规**：合并到 `main` 即触发 `build` + `deploy`（默认到 staging）
- **生产**：手动 Promote（见 GitLab → Deployments → promote）
- **灰度**：先发 Canary 实例，确认无异常再 100% rollout（详见 [08 灰度发布](./08-canary-rollout.md)）

---

## 6.6 回滚

最简单：re-deploy 上一个 commit：

```bash
# 在部署机
cd /opt/coder-micro-services
git checkout <previous-sha>
docker-compose -f infrastructure/docker-compose.yml pull
docker-compose -f infrastructure/docker-compose.yml up -d
```

镜像保留策略：registry 保留最近 10 个 tag。

---

## 6.7 发布检查清单（pre-deploy）

- [ ] 所有 CI gate 在自己的 MR 上已绿
- [ ] 已跑过完整 E2E（api + ui）
- [ ] 数据库迁移兼容（forward-only，已有 `V{N+1}__*.sql`）
- [ ] 文档 / Spec / OpenAPI 已同步更新
- [ ] 关键指标基线已记录（QPS / 错误率 / P99）
- [ ] 回滚方案明确（上一个 commit 可用）

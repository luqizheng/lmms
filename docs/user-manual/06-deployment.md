# 06. 部署

> 镜像怎么出、CI 怎么走、Promote 到生产。

---

## 6.1 构建镜像

```bash
# 全部服务
pnpm run docker:build

# 单服务
pnpm run docker:build:iam-service

# 镜像会被打 tag 推送到内网仓库：
#   zhcoder-docker-registry.com:8000/micro-server/<service>:<commit-sha>
```

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

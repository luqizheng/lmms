# CI Variables & 部署回填清单

> 状态：2026-08 起部署流水线从「runner 内执行 docker-compose」切到「CI rsync pre-staging 目录 + SSH 到部署机 + 远程 deploy-staging.sh」。
> 每个 commit SHA 一个 self-contained 目录，CI rsync 到 staging 后用 commit-pinned 镜像起服，不再 :latest。
> 本文件给 ops / 仓库维护者一份**可复制粘贴**的回填清单，按此配置即可让 GitLab CI 跑通。

---

## 1. 设计要点速览

```
GitLab CI（gitlab-ci-04 共享 docker runner）
        │
        │  build_images:pre-staging → 重建全部 9 个 + frontend 镜像
        │                            （scripts/build-docker.js，无 incremental 优化）
        │  release:pre-staging → 打 release/staging-<sha>/ 目录（artifact）
        │  deploy:pre-staging → rsync 目录 + SSH 调 deploy-staging.sh
        ▼
kudi@192.168.2.128 (STAGING_HOST)
    │
    │  跑 <STAGING_PROJECT_DIR>/pre-staging/<sha>/scripts/deploy-staging.sh
    │    ├─ preflight（docker compose v2.20+、compose 文件齐全）
    │    ├─ docker compose pull（commit-pinned 镜像）
    │    ├─ 滚动 up + healthcheck 门禁
    │    ├─ frontend:8080/health 端点验证
    │    └─ docker image prune
    ▼
frontend (8080:80)  ←── 唯一 host 公开端口
    │
    └─ container network → api-gateway:3000  (内部不通 host)
                          → token-service / iam-service / 等
```

- **公共入口**：仅 `http://192.168.2.128:8080`（frontend nginx → api-gateway:3000 内部反代）
- **api-gateway:3000 在 prod 下不暴露 host**（被 `docker-compose.prod.yml` 的 `!override []` 清掉）
- **业务后端端口**（8081/8082/8083/...）：同理，仅容器内部可达
- **mysql / redis / rabbitmq / consul**：内部组件，host 完全不可达
- **observability 端口**（9090/3001/16686/4317/4318/9093）：保留（ops 直连用）
- **CI 拓扑**：所有 stage（test / build / release / deploy / e2e / publish）都跑在 gitlab-ci-04
  共享 docker runner 上。E2E 通过 `setsid ssh -N` 反向隧道访问业务端口
  （`SSH_PRIVATE_KEY` 同样用于此）。
- **关键脚本**：
  - `scripts/release-staging.js` — 打 pre-staging 目录
  - `infrastructure/scripts/deploy-staging.sh` — 部署 pre-staging 目录（commit-pinned）
  - `infrastructure/scripts/deploy-remote.sh` — **旧**（git checkout + 本地 build，已被 deploy:pre-staging 取代；保留供 ops 手动）

---

## 2. GitLab Project → Settings → CI/CD → Variables 回填

逐项填进去（**所有 non-masked 项都不含密钥本体**，密钥只走 `SSH_PRIVATE_KEY`）：

| Key | Type | Masked | Protected | 默认值（可不填） | 说明 |
|---|---|---|---|---|---|
| `SSH_PRIVATE_KEY` | Variable | ✅ | ✅ | — | deploy:pre-staging 走 SSH rsync + bash 远程执行，**仍需要**。OPENSSH/PEM 格式私钥，对应 `kudi@STAGING_HOST`。E2E 反向隧道也用这个。 |
| `STAGING_HOST` | Variable | ❌ | ✅ | `192.168.2.128` | 部署机 IP；deploy:pre-staging SSH 目标；E2E 反向隧道目标 |
| `STAGING_DEPLOY_USER` | Variable | ❌ | ✅ | `kudi` | deploy:pre-staging SSH 用户 |
| `STAGING_PROJECT_DIR` | Variable | ❌ | ✅ | `/home/kudi/coder-micro-services` | pre-staging 目录父目录；实际路径 `${STAGING_PROJECT_DIR}/pre-staging/${CI_COMMIT_SHORT_SHA}/` |
| `STAGING_SSH_PORT` | Variable | ❌ | ✅ | `22` | SSH 端口；E2E setsid ssh -N 隧道也用这个 |
| `STAGING_BASE_URL` | Variable | ❌ | ✅ | `http://${STAGING_HOST}:8080` | frontend 公共入口；e2e 全部走这个 |
| `IAM_DEV_BT` | Variable | ✅ | ✅ | — | iam-service 的 service-jwt bootstrap token（prod 强随机） |
| `TOKEN_DEV_BT` | Variable | ✅ | ✅ | — | token-service bootstrap |
| `NOTIFICATION_DEV_BT` | Variable | ✅ | ✅ | — | notification-service bootstrap |
| `AUDIT_DEV_BT` | Variable | ✅ | ✅ | — | audit-service bootstrap |
| `CONFIG_DEV_BT` | Variable | ✅ | ✅ | — | config-service bootstrap |
| `FILE_DEV_BT` | Variable | ✅ | ✅ | — | file-service bootstrap |
| `PRUE_DEV_BT` | Variable | ✅ | ✅ | — | prue_backend bootstrap |
| `E2E_DB_PASSWORD` | Variable | ✅ | ✅ | — | MySQL root 密码（e2e 用，需与 `.env.prod` 一致） |

**Tag 发布流水线专用（`v1.3.0` 等 tag 触发时跑）**：

| Key | Type | Masked | Protected | 默认值 | 说明 |
|---|---|---|---|---|---|
| `KELLNR_TOKEN` | Variable | ✅ | ✅ | — | Rust crates 发布到 kellnr (`http://192.168.2.12:16444`) 的凭证 |
| `MAVEN_USERNAME` | Variable | ❌ | ✅ | `reposilite` | Java SDK 发到内网 Reposilite (`http://192.168.2.12:18181/releases`) — 一般填 `reposilite` |
| `MAVEN_PASSWORD` | Variable | ✅ | ✅ | — | 同上密码 |
| `NPM_TOKEN` | Variable | ✅ | ✅ | — | `@digua/dist-tools` 发到内网 npm (`http://npm.kudi.repos`) 的凭证 |

> `MAVEN_REPO_URL` / `NUGET_REPO_URL` 是内网地址，无敏感，已在 yaml 顶部硬编码；如需调整改 yaml。Java job 通过 `cp .gitlab/ci-maven-settings.xml ~/.m2/settings.xml` 注入到 Maven，凭证走 `${env.MAVEN_USERNAME}` / `${env.MAVEN_PASSWORD}` 插值（详见 `.gitlab/ci-maven-settings.xml`）。

> **完全不需要** `DEPLOY_PASSWORD`（已删除）；密钥直接走 `SSH_PRIVATE_KEY`。
> DEPLOY_SERVER / DEPLOY_USER 旧名也已删除。

---

## 3. 部署机一次性初始化

> deploy:pre-staging 走普通 docker runner + SSH，不需要在 staging 上注册 gitlab-runner。
> 旧 deploy-staging shell runner（`tags: [deploy-staging]`）已被取代，可选保留作回滚路径。

在 `kudi@192.168.2.128` 上以用户身份（不是 root）：

```bash
# 1. 准备 .env.prod（deploy-staging.sh 会 cd 到 STAGING_PROJECT_DIR/pre-staging/<sha>/）
#    pre-staging 目录是 self-contained 的；只需保证部署机有 docker compose v2.20+ 即可。
cp infrastructure/.env.prod.example ~/.env.prod
$EDITOR ~/.env.prod
# 填入所有 __SET_BY_OPS__ 占位（密码、强随机的 bootstrap token）

# 2. 确认 docker compose v2.20+
docker compose version | head -1
# v2.20+ 才有 !override 标签支持。

# 3. 部署机不需要先 mkdir 任何目录 —— pre-staging 任务会 rsync 创建
#    /home/kudi/coder-micro-services/pre-staging/<commit_sha>/
```

CI 第一次跑 deploy:pre-staging 会做：
- 在 gitlab-ci-04 共享 docker runner 上下载 release:pre-staging artifact
- `rsync -a --delete` 把目录同步到 `${STAGING_PROJECT_DIR}/pre-staging/${CI_COMMIT_SHORT_SHA}/`
- SSH 到 staging，调 `STAGING_COMMIT=$CI_COMMIT_SHORT_SHA bash .../scripts/deploy-staging.sh`
- 部署脚本依次：`docker compose pull`（commit-pinned 镜像）→ 滚动 up → `/health` 校验

---

## 4. SSH key 配对（部署机侧）

deploy:pre-staging + e2e_migration_state_test 都走 SSH，需要把 GitLab Project 的
`SSH_PRIVATE_KEY` 对应的公钥放进 `kudi@192.168.2.128` 的 `~/.ssh/authorized_keys`：

```bash
# 在部署机（kudi@192.168.2.128）：
mkdir -p ~/.ssh && chmod 700 ~/.ssh
touch ~/.ssh/authorized_keys && chmod 600 ~/.ssh/authorized_keys
# 把 SSH_PRIVATE_KEY 对应的公钥追加进去（密钥由 ops 持有）
cat >> ~/.ssh/authorized_keys <<'EOF'
ssh-ed25519 AAAA... kudi@gitlab-ci-deploy-key
EOF
```

CI 端会通过 `ssh-keyscan` 把 host key 加进 `known_hosts`，无需提前 trust。

---

## 5. 验证流水线

**主线（push 到 `main` 分支）**：

| Stage | Job | 期望 |
|---|---|---|
| test | rust_test, service_standard_check, spec_check, ... | 全部 PASS |
| build | `build_images:pre-staging` | 9 个服务 + frontend 镜像按 `$CI_COMMIT_SHORT_SHA` tag 推 registry（无 incremental 优化：每个 commit 全部重建，保证所有 commit SHA tag 都存在） |
| build | `release:pre-staging` | `release/staging-$CI_COMMIT_SHORT_SHA/` 目录生成；artifact 3 天过期 |
| deploy | `deploy:pre-staging` | rsync 目录到 staging → SSH 调 `deploy-staging.sh` → curl frontend:8080/health = `{"status":"ok"}` |
| e2e | e2e_api_test | 经 SSH-tunnel 走 frontend:8080 → api-gateway:3000 |
| e2e | e2e_ui_test | Playwright 走 frontend:8080 |
| e2e | e2e_migration_state_test | 通过 7 个 SSH-tunnel 端口直连业务服务 |

**Tag 发布流水线（push tag `v1.3.0` 触发全部 4 个 publish + 1 个 release job）**：

| Stage | Job | 期望 |
|---|---|---|
| publish | `sdk_publish` | Java SDK 发到 `http://mvn.kudi.repos`（`io.digua.*` 包，按版本号） |
| publish | `rust_publish` | 17 个 libs/* crate + 7 个 sdk/rust/* crate 发到 kellnr（按版本号 + 拓扑顺序） |
| publish | `build_images:tagged` | 9 个服务 + frontend 镜像按 `$VERSION` tag 推 `zhcoder-docker-registry.com:8000/micro-server/<svc>:1.3.0` + `:latest`（buildx push） |
| publish | `publish_npm` | `@digua/dist-tools@1.3.0` 发到 `http://npm.kudi.repos` |
| release | `release:dist` | 等上面 4 个 job 全部成功 → 跑 `node scripts/release.js 1.3.0 --no-images` 产离线分发包 artifact 下载 |

> 验证 tag 流水线：在本地 `git tag v1.3.0 && git push origin v1.3.0`，GitLab 自动触发 pipeline 跑所有 stage。

**手动发布（CI 太慢 / 临时验证时）**：

```bash
# 1. 在仓库根目录，构建并推送所有镜像（--push 不留本地）
node scripts/build-docker.js -v <commit_sha> --push \
    api-gateway token-service iam-service notification-service \
    audit-service config-service file-service prue-admin-backend \
    frontend

# 2. 生成 pre-staging 目录（仅 docker-compose + deploy 脚本，不打镜像）
node scripts/release-staging.js --commit <commit_sha> \
    --output release/staging-<commit_sha>/

# 3. rsync 目录 + SSH 调部署脚本
rsync -a release/staging-<commit_sha>/ kudi@192.168.2.128:~/pre-staging/<commit_sha>/
ssh kudi@192.168.2.128 "STAGING_COMMIT=<commit_sha> bash ~/pre-staging/<commit_sha>/scripts/deploy-staging.sh"

# 4. 部署脚本会自动：
#    - pull commit-pinned 镜像
#    - 滚动 up + healthcheck
#    - curl frontend:8080/health 校验
```

---

## 7. 紧急回滚

**Pre-staging 部署回滚**（commit-pinned 镜像，回滚到上一个 commit）：

```bash
# 在部署机，把 staging 切回上一个已部署 commit：
ls -1 ~/coder-micro-services/pre-staging/ | sort
# 选目标 commit sha，调对应目录的 deploy-staging.sh
cd ~/coder-micro-services/pre-staging/<previous_sha>/
STAGING_COMMIT=<previous_sha> bash scripts/deploy-staging.sh
# 脚本会重新 pull 对应镜像 → 滚动重启
```

或重新跑 CI pipeline（push 一个 revert commit / 重跑 main pipeline）让 release:pre-staging + deploy:pre-staging 自动滚回。

**prod overlay 紧急下线**：

prod overlay 是 dev base 的**叠加**层。单独 rm 它并不会停服务：
```bash
# 在部署机，让 prod overlay 临时失效、回退到 dev base（3000 等端口重新暴露 — 仅紧急用）
cd ~/coder-micro-services
sudo docker compose -f infrastructure/docker-compose.yml up -d
# 此时 3000/8082/... 重新 bind host，业务上 = dev 同等待遇
```

正确下线：
```bash
sudo /home/kudi/coder-micro-services/infrastructure/scripts/stop-prod.sh
```

---

## 8. 已知未覆盖（留待后续阶段）

- **nginx TLS terminator**（443/SSL）：当前仅 8080/HTTP。spec `2026-07-06-prod-compose-overlay-design.md` 写明推到 staging 后再做。
- **K8s / Helm**：完全 out-of-scope。
- **Resource limits / read-only rootfs**：同 spec out-of-scope。
- **Consul ACL token / service-jwt 私钥持久化**：独立 security spec。

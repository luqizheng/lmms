# CI Variables & 部署回填清单

> 状态：2026-08 起部署流水线收敛为「continuous staging flow」（per-commit SHA + atomic symlink swap）。
> `build_images:staging` 按 `$CI_COMMIT_SHORT_SHA` 推 8 个服务 + mysql-replica 镜像（frontend 已随 PRISM console 移除）；
> `deploy:staging` SSH 到部署机，调 `load-images.js --mode=pull` 拉 commit-pinned 镜像后调 `swap-staging-active.sh` 切 active 软链。
> 每个 commit SHA 一个 immutable 部署目录；e2e 失败自动 swap 回 staging-prev。
> 本文件给 ops / 仓库维护者一份**可复制粘贴**的回填清单，按此配置即可让 GitLab CI 跑通。

---

## 1. 设计要点速览

```
GitLab CI（gitlab-ci-04 共享 docker runner）
        │
        │  build_images:staging → 重建全部 8 服务 + mysql-replica 镜像
        │                         （coder build images + 单独 buildx mysql-replica）
        │                         buildx push 到内网 registry，tag = $CI_COMMIT_SHORT_SHA
        │  release:staging     → 跑 release-deploy.js --no-images 产骨架（artifact ~1MB）
        │  deploy:staging      → tar over ssh 传骨架到部署机
        │                         ssh 'load-images.js --mode=pull' 拉 commit-pinned 镜像
        │                         ssh 'swap-staging-active.sh <sha>' 原子切换 active 软链
        ▼
kudi@192.168.2.31 (STAGING_HOST)
        │
        │  <STAGING_PROJECT_DIR>/
        │  ├── staging-active -> staging-<current-sha>     # 原子软链，唯一可变
        │  ├── staging-prev   -> staging-<previous-sha>    # rollback 锚点
        │  └── staging-<sha>/                              # immutable per-SHA 目录
        ▼
api-gateway (3000:3000)  ←── 唯一 host 公开端口
        │
        └─ container network → token-service / iam-service / 等
```

- **公共入口**：仅 `http://192.168.2.31:3000`（api-gateway 直连；原 frontend 8080 反代层已随 PRISM console 移除）
- **业务后端端口**（8081/8082/8083/...）：仅容器内部可达（prod overlay `!override []` 清掉 host 发布）
- **mysql / redis / rabbitmq / consul**：内部组件，host 完全不可达
- **observability 端口**（9090/3001/16686/4317/4318/9093）：保留（ops 直连用）
- **CI 拓扑**：所有 stage（test / build / release / deploy / e2e / publish）都跑在 gitlab-ci-04
  共享 docker runner 上。E2E 通过 `setsid ssh -N` 反向隧道访问业务端口
  （`SSH_PRIVATE_KEY` 同样用于此）。
- **关键脚本**：
  - `release/dist-template/scripts/release-deploy.js --no-images` — 产骨架（artifact ~1MB）
  - `release/dist-template/bin/load-images.js --mode=pull` — 从 registry 拉 commit-pinned 镜像
  - `infrastructure/scripts/swap-staging-active.sh` — 原子切 active 软链（deploy + e2e rollback 共用）
  - `infrastructure/scripts/staging-gc.sh` — 清理老 staging-<sha> 目录（cron.daily）
  - `infrastructure/scripts/deploy-remote.sh` — **旧**（git checkout + 本地 build 模式，已被 continuous staging 取代；保留供 ops 紧急手动使用）

---

## 2. GitLab Project → Settings → CI/CD → Variables 回填

逐项填进去（**所有 non-masked 项都不含密钥本体**，密钥只走 `SSH_PRIVATE_KEY`）：

| Key | Type | Masked | Protected | 默认值（可不填） | 说明 |
|---|---|---|---|---|---|
| `SSH_PRIVATE_KEY` | Variable | ✅ | ✅ | — | deploy:staging 走 SSH 远程执行 + tar 传输，**仍需要**。OPENSSH/PEM 格式私钥，对应 `kudi@STAGING_HOST`。E2E 反向隧道也用这个。 |
| `STAGING_HOST` | Variable | ❌ | ✅ | `192.168.2.31` | 部署机 IP；deploy:staging SSH 目标；E2E 反向隧道目标 |
| `STAGING_DEPLOY_USER` | Variable | ❌ | ✅ | `kudi` | deploy:staging SSH 用户 |
| `STAGING_PROJECT_DIR` | Variable | ❌ | ✅ | `/home/kudi/coder-micro-services` | staging 目录父目录；含 `staging-active` symlink + `staging-<sha>/` 目录 |
| `STAGING_SSH_PORT` | Variable | ❌ | ✅ | `22` | SSH 端口；E2E setsid ssh -N 隧道也用这个 |
| `STAGING_BASE_URL` | Variable | ❌ | ✅ | `http://${STAGING_HOST}:3000` | api-gateway 公共入口（唯一 host 公开端口）；e2e 全部走这个 |
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
| `NPM_TOKEN` | Variable | ✅ | ✅ | — | `@digua/contracts` + `@digua/http` 发到内网 npm (`http://npm.kudi.repos`) 的凭证 |

> `MAVEN_REPO_URL` / `NUGET_REPO_URL` 是内网地址，无敏感，已在 yaml 顶部硬编码；如需调整改 yaml。Java job 通过 `cp .gitlab/ci-maven-settings.xml ~/.m2/settings.xml` 注入到 Maven，凭证走 `${env.MAVEN_USERNAME}` / `${env.MAVEN_PASSWORD}` 插值（详见 `.gitlab/ci-maven-settings.xml`）。

> **完全不需要** `DEPLOY_PASSWORD`（已删除）；密钥直接走 `SSH_PRIVATE_KEY`。
> DEPLOY_SERVER / DEPLOY_USER 旧名也已删除。

---

## 3. 部署机一次性初始化

> deploy:staging 走普通 docker runner + SSH，不需要在 staging 上注册 gitlab-runner。

在 `kudi@192.168.2.31` 上以用户身份（不是 root）：

```bash
# 1. 确认 docker compose v2.20+
docker compose version | head -1
# v2.20+ 才有 !override 标签支持。

# 2. 部署机不需要先 mkdir 任何目录 —— deploy:staging 会 SSH 创建
#    /home/kudi/coder-micro-services/staging-<commit_sha>/
```

CI 第一次跑 deploy:staging 会做：
- 在 gitlab-ci-04 共享 docker runner 上下载 release:staging artifact（~1MB 骨架）
- tar over ssh 传目录到 `${STAGING_PROJECT_DIR}/staging-${CI_COMMIT_SHORT_SHA}/`
- SSH 到 staging 调 `IMAGE_TAG=<sha> node bin/load-images.js --mode=pull` 拉 commit-pinned 镜像
- SSH 调 `swap-staging-active.sh <sha>` 原子切换 active 软链
- curl `STAGING_BASE_URL/health` 验证

---

## 4. SSH key 配对（部署机侧）

deploy:staging + e2e_migration_state_test 都走 SSH，需要把 GitLab Project 的
`SSH_PRIVATE_KEY` 对应的公钥放进 `kudi@192.168.2.31` 的 `~/.ssh/authorized_keys`：

```bash
# 在部署机（kudi@192.168.2.31）：
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
| build | `build_images:staging` | 8 个服务 + mysql-replica 镜像按 `$CI_COMMIT_SHORT_SHA` tag 推 registry |
| build | `release:staging` | 骨架目录生成；artifact 3 天过期 |
| deploy | `deploy:staging` | tar over ssh 传目录 → `load-images.js --mode=pull` → `swap-staging-active.sh <sha>` → curl health = `{"status":"ok"}` |
| e2e | `e2e:staging` | SSH 反向隧道 + 跑 e2e；失败 swap 回 staging-prev |

**Tag 发布流水线（push tag `v1.3.0` 触发全部 4 个 publish + 1 个 release job）**：

| Stage | Job | 期望 |
|---|---|---|
| publish | `sdk_publish` | Java SDK 发到 `http://mvn.kudi.repos`（`io.digua.*` 包，按版本号） |
| publish | `rust_publish` | 17 个 libs/* crate + 7 个 sdk/rust/* crate 发到 kellnr（按版本号 + 拓扑顺序） |
| publish | `build_images:tagged` | 8 个服务镜像按 `$VERSION` tag 推 `zhcoder-docker-registry.com:8000/micro-server/<svc>:1.3.0` + `:latest`（buildx push；frontend 已移除） |
| publish | `publish_packages` | `@digua/contracts` + `@digua/http` 按 tag 版本发到 `http://npm.kudi.repos`（`check-packages-publish.js` 前置校验） |
| release | `release:dist` | 等上面 4 个 job 全部成功 → 跑 `node scripts/release.js 1.3.0 --no-images` 产离线分发包 artifact 下载 |

> 验证 tag 流水线：在本地 `git tag v1.3.0 && git push origin v1.3.0`，GitLab 自动触发 pipeline 跑所有 stage。

**手动部署（CI 不可用时 fallback）**：

```bash
# 查看当前 active 版本
ssh kudi@192.168.2.31 "readlink -f /home/kudi/coder-micro-services/staging-active"

# 查看所有已部署版本
ssh kudi@192.168.2.31 "ls -lt /home/kudi/coder-micro-services/staging-* | head -10"

# 手动部署一个 SHA（替代 CI）
ssh kudi@192.168.2.31 "
  REMOTE=/home/kudi/coder-micro-services/staging-<sha>
  cd \$REMOTE && \
  IMAGE_TAG=<sha> node bin/load-images.js --mode=pull && \
  STAGING_BASE_DIR=/home/kudi/coder-micro-services bash infrastructure/scripts/swap-staging-active.sh <sha>
"

# 手动 rollback
ssh kudi@192.168.2.31 "
  STAGING_BASE_DIR=/home/kudi/coder-micro-services \
    bash /home/kudi/coder-micro-services/staging-<old-sha>/infrastructure/scripts/swap-staging-active.sh <old-sha>
"

# 清理老目录（默认保留最近 5 个 + 保护 active/prev）
ssh kudi@192.168.2.31 "bash infrastructure/scripts/staging-gc.sh"
ssh kudi@192.168.2.31 "bash infrastructure/scripts/staging-gc.sh --keep 10"  # 改阈值

# 安装 GC 到 cron
ssh kudi@192.168.2.31 "sudo install -m755 infrastructure/scripts/staging-gc.sh /etc/cron.daily/staging-gc"
```

---

## 6. 镜像缺失同步 SOP

`deploy:staging` 在 staging 服务器上跑 `release/dist-template/bin/load-images.js --mode=pull`，从内网 registry (`${STAGING_REGISTRY}/${STAGING_BASE_PATH}`) 拉取 compose 里所有 `image:` 引用。**任一缺失即整个 deploy job fail (exit 1)**——这是 hard fail 设计：业务服务镜像缺失必须 fail-fast。

但**第三方基础镜像**（prometheus / alertmanager / grafana / jaeger / otel-collector / consul / mysql / redis / rabbitmq 等）偶尔会因镜像仓库 GC / 重置出现 tag 不在仓库里，且与本次 PR 内容**无关**。这种场景的修复 SOP 在此。

> 历史案例：[job 24913](http://192.168.2.16/template/low-memory-micro-services/-/jobs/24913)（commit c63679fc）— `prom/alertmanager:v0.27.0` 在内网仓库缺失，整个 deploy 红标但与 cargo-deny 升级无关。

**一键同步（推荐）**：

`infrastructure/scripts/sync-staging-images.sh` 把"缺什么 → 一行命令补上"沉淀为 ops 资产。**在 staging 服务器 (192.168.2.31) 上执行**：

```bash
# 1) 先 dry-run 看会做什么（不实际拉/推）
bash infrastructure/scripts/sync-staging-images.sh --dry-run \
    --images "prom/alertmanager:v0.27.0"

# 2) 实际同步
bash infrastructure/scripts/sync-staging-images.sh \
    --images "prom/alertmanager:v0.27.0"

# 3) 也可以从 stdin 喂入（方便从 CI 失败日志批量提取）
bash infrastructure/scripts/sync-staging-images.sh <<'EOF'
prom/alertmanager:v0.27.0
mysql:8.0
EOF
```

脚本行为：
- **幂等**：先 `docker manifest inspect` 内网仓库，已在则跳过；不存在才 pull + tag + push
- **默认上游** `docker.io`，可通过 `UPSTREAM_REGISTRY` env 覆盖
- **模式**：默认 `sync`；支持 `--check`（只检查不推）/ `--dry-run`（只看不执行）/ `--from-tar <file>`（staging 服务器不通 docker.io 时用离线 tarball 喂入）

**手动 fallback（无脚本可用时）**：

```bash
# 在 staging 服务器，三行命令即可
docker pull prom/alertmanager:v0.27.0
docker tag prom/alertmanager:v0.27.0 \
  zhcoder-docker-registry.com:8000/official/prom/alertmanager:v0.27.0
docker push zhcoder-docker-registry.com:8000/official/prom/alertmanager:v0.27.0
```

注意 `${BASE_REGISTRY}` 默认展开为 `zhcoder-docker-registry.com:8000/official/`（带尾斜杠后 `official/`，不要拼成 `officialprom`）。

**同步后重跑 deploy**：

镜像补齐后去 GitLab UI 重跑失败的 job 即可，**不需要新 commit**：

- 打开失败的 pipeline（[示例](http://192.168.2.16/template/low-memory-micro-services/-/pipelines/6621)）
- 点 `deploy:staging` → **Retry**
- 想跑完整 `deploy → e2e` 链路：pipeline 页右上角 **Retry** 下拉 → **Retry with same variables**

### 已知用过的第三方基础镜像清单

`load-images.js --mode=pull` 在 staging 实际拉取的第三方基础镜像共 **11 个**（compose 文件 → 镜像名）：

| Compose 文件 | 镜像 |
|---|---|
| `docker-compose-infra.yml` | `hashicorp/consul:2.0.1`、`gliderlabs/registrator:master`、`redis:7-alpine`、`mysql:8.0`、`rabbitmq:3.13-management-alpine` |
| `docker-compose-observability.yml` | `prom/prometheus:v2.51.0`、`prom/alertmanager:v0.27.0`、`grafana/grafana:10.4.0`、`hashicorp/consul-template:0.32.0`、`otel/opentelemetry-collector-contrib:0.96.0`、`jaegertracing/all-in-one:1.55` |

不在 sync 范围：
- **业务服务镜像**（`micro-server/<svc>:<sha>` 共 8 个 + `coder/mysql-replica:8.0`）—— `build_images:staging` 自己 push，sync 脚本不动它们
- **`grafana/loki:3.3.2` / `grafana/promtail:3.3.2`** —— 在 `docker-compose-observability.staging.yml` 里，但 `load-images.js` 当前不扫此文件，由 ops 手动 `up -d` 时拉

### 批量同步（防患于未然）

staging 服务器 (192.168.2.31) 上一行命令把 11 个第三方基础镜像全跑一遍（幂等，已在内网的自动跳过）：

```bash
bash infrastructure/scripts/sync-staging-images.sh --images "
  hashicorp/consul:2.0.1
  gliderlabs/registrator:master
  redis:7-alpine
  mysql:8.0
  rabbitmq:3.13-management-alpine
  prom/prometheus:v2.51.0
  prom/alertmanager:v0.27.0
  grafana/grafana:10.4.0
  hashicorp/consul-template:0.32.0
  otel/opentelemetry-collector-contrib:0.96.0
  jaegertracing/all-in-one:1.55
"
```

新增第三方镜像时（往 `docker-compose-infra.yml` / `docker-compose-observability.yml` 加 `image:` 行）：先 `--check` 一次确认在内网，不在则 sync；并把新镜像加到上面清单里（保持文档与 compose 同步）。

---

## 7. 紧急回滚

**Continuous staging 自动回滚**：e2e 失败时 `swap-staging-active.sh` 会把 staging-prev 重新链为 active，无需人工介入。

**手动回滚到上一个 commit**：
```bash
# 在部署机，把 staging 切回上一个已部署 commit：
ls -1 ~/coder-micro-services/staging-* -d | sort
# 选目标 commit sha，调对应目录的 swap-staging-active.sh
STAGING_BASE_DIR=/home/kudi/coder-micro-services \
  bash /home/kudi/coder-micro-services/staging-<previous-sha>/infrastructure/scripts/swap-staging-active.sh <previous-sha>
```

或重新跑 CI pipeline（push 一个 revert commit / 重跑 main pipeline）让 deploy:staging 自动重新部署。

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

- **nginx TLS terminator**（443/SSL）：当前仅 3000/HTTP（api-gateway 直连）。spec `2026-07-06-prod-compose-overlay-design.md` 写明推到 staging 后再做。
- **K8s / Helm**：完全 out-of-scope。
- **Resource limits / read-only rootfs**：同 spec out-of-scope。
- **Consul ACL token / service-jwt 私钥持久化**：独立 security spec。

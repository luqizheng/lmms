# CI Variables & 部署回填清单

> 状态：2026-08 起部署流水线从「runner 内执行 docker-compose」切到「runner SSH 到部署机 + 远程 deploy-remote.sh」。
> 本文件给 ops / 仓库维护者一份**可复制粘贴**的回填清单，按此配置即可让 GitLab CI 跑通。

---

## 1. 设计要点速览

```
GitLab runner（deploy-staging, shell executor）
        ▲
        │  runner 进程本地 g it checkout + 直接执行 deploy-remote.sh
        │
kudi@192.168.2.128 (STAGING_HOST) —— runner host = deploy host
    │
    │  跑 infrastructure/scripts/deploy-remote.sh
    │    ├─ preflight（docker compose v2.20+、关键 vars 非空、PATH 探测）
    │    ├─ docker compose pull (prod overlay)
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
- **CI 拓扑**：deploy / deploy_smoke 跑在 staging 上以 `kudi` 注册的 shell runner；
  test / build / e2e 仍在 gitlab-ci-04 共享 docker runner 上。E2E 通过
  `setsid ssh -N` 反向隧道访问业务端口（需要 SSH_PRIVATE_KEY 仍用于
  gitlab-ci-04 一侧）。

---

## 2. GitLab Project → Settings → CI/CD → Variables 回填

逐项填进去（**所有 non-masked 项都不含密钥本体**，密钥只走 `SSH_PRIVATE_KEY`）：

| Key | Type | Masked | Protected | 默认值（可不填） | 说明 |
|---|---|---|---|---|---|
| `SSH_PRIVATE_KEY` | Variable | ✅ | ✅ | — | **自 deploy-staging shell runner 切换后已不需要**。旧 SSH-based deploy 用的私钥（OPENSSH/PEM 格式）。保留仅作回滚兜底，可清理。 |
| `SSH_KNOWN_HOSTS` | Variable | ❌ | ✅ | — | **自 deploy-staging shell runner 切换后已不需要**。保留仅作回滚兜底，可清理。 |
| `STAGING_HOST` | Variable | ❌ | ✅ | `192.168.2.128` | runner host IP/域名；E2E job 经 SSH 隧道时仍用 |
| `STAGING_DEPLOY_USER` | Variable | ❌ | ✅ | `kudi` | **deprecated**：deploy job 现在直接跑在 staging 本机，不再 SSH。保留仅作回滚兜底 |
| `STAGING_PROJECT_DIR` | Variable | ❌ | ✅ | `/home/kudi/coder-micro-services` | **deprecated**：shell runner 自动 git checkout 到 build_dir，不再用这个目录。保留仅作历史兼容 |
| `STAGING_SSH_PORT` | Variable | ❌ | ✅ | `22` | E2E setsid ssh -N 隧道用，仍需要 |
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

## 3. 注册 deploy-staging shell runner

> **必做一次。** deploy job（`tags: [deploy-staging]`）现在直接跑在 staging 本机上，不再 SSH 跨主机；没有这个 runner 时 deploy job 会一直 pending。

在 `kudi@192.168.2.128` 上以 `kudi` 身份（**不是 root**）：

```bash
# 1. 安装 gitlab-runner（Debian/Ubuntu；用项目内网的 apt 镜像）
#    GitLab 官方 repo 也可：https://docs.gitlab.com/runner/install/linux-repository.html
sudo apt-get update
sudo apt-get install -y gitlab-runner
# 注意：要保持 gitlab-runner 版本 ≥ 17.x，与 GitLab 19.x 兼容

# 2. 注册 runner
sudo gitlab-runner register
# 提示输入：
#   GitLab URL      : https://gitlab.example.com/   (或本仓库对应内网 GitLab URL)
#   Registration token: Settings → CI/CD → Runners → Register an instance runner
#   Description      : deploy-staging on 192.168.2.128
#   Tags             : deploy-staging   ← 关键，只接 deploy/deploy_smoke 两个 job
#   Executor         : shell
#   Default image    : (空)

# 3. 验证 kudi 在 docker 组里（kudi 已加入组则跳过）
id kudi | tr ',' '\n' | grep -q docker || sudo usermod -aG docker kudi
# 重新登录让 group 生效

# 4. 验证 runner 在线
sudo gitlab-runner list
sudo gitlab-runner verify
# 状态显示 "Active" 且 tag=deploy-staging 即 OK
```

注册之后，runner 默认以 `gitlab-runner` 用户运行；如要让它以 `kudi` 身份跑（与 `id kudi` 的 docker 组匹配，需进入 ~/.ssh + docker.sock），修改 `/etc/systemd/system/gitlab-runner.service.d/override.conf`：

```ini
[Service]
User=kudi
Group=kudi
```

然后 `sudo systemctl daemon-reload && sudo systemctl restart gitlab-runner`。

## 4. 部署机一次性的初始化

在 `kudi@192.168.2.128` 上以用户身份（不是 root）：

```bash
# 1. 准备 .env.prod（runner 会 cd 到 build_dir 并 PROJECT_ROOT 推断到 git 根）
#    不再需要手动 mkdir ~/coder-micro-services —— git checkout 自动落到
#    /home/kudi/builds/template/low-memory-micro-services/
cp infrastructure/.env.prod.example infrastructure/.env.prod
$EDITOR infrastructure/.env.prod
# 填入所有 __SET_BY_OPS__ 占位（密码、强随机的 bootstrap token）

# 2. 确认 docker compose v2.20+
docker compose version | head -1
# v2.20+ 才有 !override 标签支持。

# 3. 预拉镜像（可选，部署首次也会再 pull）
docker compose -f infrastructure/docker-compose.yml -f infrastructure/docker-compose.prod.yml pull
```

CI 第一次跑 deploy job 会做：
- gitlab-runner 自动 git checkout 到 build_dir
- 在 staging 本机跑 `bash infrastructure/scripts/deploy-remote.sh`（首次会 `pull` 所有镜像）

---

## 5. SSH key 配对（部署机侧） — **deprecated**

> 自 deploy-staging shell runner 切换后，deploy job 不再使用 SSH key。
> 本节保留仅为回滚到 `tags: [docker]` SSH 模式时参考。E2E 段 `setsid ssh -N`
> 仍可能需要 SSH key（GitLab Runner 在 gitlab-ci-04 上，未迁移）。

`.gitlab-ci.yml` 的 deploy job 旧路径使用 `SSH_PRIVATE_KEY`（含在你这次贴给我的私钥）。该私钥对应 `kudi@micro-server` 主机签名。

**把公钥写进部署机**：
```bash
# 在部署机（kudi@192.168.2.128）：
ssh-keyscan -H 192.168.2.128 > ~/.ssh/known_hosts_192   # 可选；本地信任
# 推荐用 deploy key：在部署机的 ~/.ssh/authorized_keys 加入对应公钥
```

如果用 SSH key 模式：
```bash
# 已用 SSH key，无需密码登录；CI 自动 bypass sudo 提示
```

如果仅有密码（兜底路径，目前未启用）：
```bash
# 旧 DEPLOY_PASSWORD 路径：CI 会 install sshpass 然后用 SSHPASS env。
# 推荐直接切到 SSH key（更安全 + 可审计）
```

---

## 6. 验证流水线

**主线（push 到 `main` 分支）**：

| Stage | Job | 期望 |
|---|---|---|
| test | rust_test, service_standard_check, spec_check, ... | 全部 PASS |
| build | 9 个 build_<svc> + build_frontend | 全部 PASS（images push 到 zhcoder-docker-registry，tag = short-sha + latest） |
| deploy | deploy | SSH 到 192.168.2.127 → 滚动重启 → curl frontend:8080/health = `{"status":"ok"}` |
| deploy | deploy_smoke | 同上端点二次确认 |
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

---

## 7. 紧急回滚

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

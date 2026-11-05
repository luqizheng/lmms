# Release Tag Pipeline Runbook

> 2026-08 起：打 tag → CI 自动跑完 Java SDK / Rust crates / Docker 镜像 / npm / 离线分发包。
> 本文档给 ops / 仓库维护者一份**实操**手册：准备工作 → 打 tag → 监控 → 失败回滚。

---

## 0. 触发条件

```
本地
$ git tag v1.3.0
$ git push origin v1.3.0
```

只要 tag 形如 `vX.Y.Z`（**必须带 `v` 前缀**），`.gitlab-ci.yml` 的 5 个 publish/release job 全部触发。

> **注意**：tag 必须从 `main` 分支切出（tag 本身是个 commit 指针）。如果你临时改了点东西想热修 → 在 main 上再发一个 commit 然后 `--force` tag；或 `--delete` 旧 tag 再 push 新 tag。

---

## 1. 跑 tag 之前必须检查（一次）

### GitLab Project → Settings → CI/CD → Variables 是否齐

| Key | 用在 | Masked? | 来源 |
|---|---|---|---|
| `SSH_PRIVATE_KEY` | 部署机 ed25519 私钥 | ✅ | ops 从 192.168.2.127 的 `~/.ssh/id_*` 取 |
| `E2E_DB_PASSWORD` | MySQL root 密码（与 `.env.prod` 一致） | ✅ | 与 192.168.2.127 的 `infrastructure/.env.prod` 中 `MYSQL_ROOT_PASSWORD` 同值 |
| `KELLNR_TOKEN` | Rust crates 发布 | ✅ | `cargo login --registry kellnr` 的 token |
| `MAVEN_USERNAME` | Java SDK 发到 maven | ❌ | 默认 `reposilite`（Reposilite 默认账号） |
| `MAVEN_PASSWORD` | Java SDK 发到 maven | ✅ | Reposilite 密码 |
| `NPM_TOKEN` | `@digua/contracts` + `@digua/http` 发到 npm | ✅ | 内网 npm registry 的 access token |

> 详细回填表见 `infrastructure/scripts/CI_VARIABLES.md` §2。

### 部署机 `192.168.2.127` 状态

```bash
ssh kudi@192.168.2.127
# 1. compose v2.20+
docker compose version | head -1

# 2. 项目目录存在（CI 会 rsync 进来）
ls -d ~/coder-micro-services || mkdir -p ~/coder-micro-services

# 3. .env.prod 已填好
cat ~/coder-micro-services/infrastructure/.env.prod | grep -v '^#'
#   所有 __SET_BY_OPS__ 必须已替换为实际值

# 4. 公钥已写到 ~/.ssh/authorized_keys（来自 SSH_PRIVATE_KEY 对应公钥）
#    如果没写，CI 会在 ssh-keyscan 时 fail / 拒绝登录
```

### runner 上有 docker + sudo

`build_images:tagged` 走 `tags: [shell]`（不是 docker executor），需要 runner 主机装了 docker engine + runner 用户有 sudo 权限。改用 docker executor 时任务里需要 docker:dind（**目前未走这条路**，shell 即可）。

---

## 2. 打 tag

```bash
# 在 main 分支最新 commit 上
git checkout main
git pull
git tag v1.3.0
git push origin v1.3.0
```

GitLab 检测到 tag push → 自动创建 pipeline → 跑所有 stage（test → build → deploy → e2e → publish → release）。

---

## 3. 监控 pipeline

GitLab UI：`http://192.168.2.16/template/low-memory-micro-services/-/pipelines?scope=tags&search=v1.3.0`

| Stage | Job | 期望用时 | 看什么 |
|---|---|---|---|
| test | rust_ci 等全部 test job | 8-15 min | 全部 PASS |
| build | `build_images:tagged` | 5-15 min | 8 个服务镜像全部推成 `:1.3.0` + `:latest`（frontend 与 mysql-replica 镜像已移除） |
| deploy | deploy:staging | 3-5 min | SSH 通；api-gateway:3000/health = `{"status":"ok"}`（唯一公共入口） |
| e2e | e2e_api_test + e2e_migration_state_test | 5-15 min | 走 `STAGING_BASE_URL`（api-gateway:3000）；UI 套件已移除 |
| **publish** | **4 个并行** ⭐ | **5-20 min** | 见下表 |
| release | release:dist | 1-2 min | 等上面 4 个完成；下载 artifact |

publish stage 4 个并行 job 的关键观察点：

| Job | 卡哪 | 怎么查 |
|---|---|---|
| `sdk_publish` | maven（`http://mvn.kudi.repos`）网络不通；`MAVEN_*` 凭证错 | 看 job 内 `mvn install -DskipTests` + curl 上传步骤输出 |
| `rust_publish` | kellnr（`http://192.168.2.12:16444`）网络不通；`KELLNR_TOKEN` 错 | 看 `cargo login` 步骤；可手动 `cargo login --registry kellnr` 校验 |
| `build_images:tagged` ⭐ | runner 上 docker buildx 没装；registry 推不进去 | 看 `docker buildx inspect --bootstrap` 输出；如果镜像已经 push 成功，会列 8 个服务 `:1.3.0` tag |
| `publish_packages` ⭐ | 内网 npm registry (`http://npm.kudi.repos`) 不可达；`NPM_TOKEN` 错；版本检查不过（`check-packages-publish.js`） | 看 `check-packages-publish.js` 输出（会列出全部阻断项）；`pnpm run packages:build` 失败看 tsc 报错 |

### 期望最终落地的 artifacts（registry 内可见）

```bash
# Docker registry：zhcoder-docker-registry.com:8000/micro-server/*:1.3.0 + :latest
# 例如：
#   api-gateway:1.3.0
#   token-service:1.3.0
#   ...
#   prue-admin-backend:1.3.0
#   （frontend 镜像已随 PRISM console 移除，不再发布）
#
# Maven: io.digua.* / com.digua.* artifacts at v1.3.0
# kellnr (Rust crates): 17 个 libs/* + 7 个 sdk/rust/* at v1.3.0
# npm (内网): @digua/contracts@1.3.0 + @digua/http@1.3.0（publish_packages；@digua/dist-tools/cms-sync 已停用不再发布）
```

---

## 4. 失败回滚

### 4.1 删 local + remote tag

```bash
git tag -d v1.3.0              # local
git push origin :refs/tags/v1.3.0   # remote
```

下游 registry 上的镜像 / artifact **仍存在**（immutable 性质），需要单独清理：

```bash
# Docker registry：登录 zhcoder-docker-registry.com:8000 后用 API 删 tag
# 通常：git tag rollback 后不必立即删；下个 tag 自然覆盖 :latest
```

### 4.2 只回退某一个 artifact

- **镜像错**：先 revert 那个 commit，重新打 tag（tag 名用 `v1.3.0-hotfix1`）
- **Rust crate 错**：crates 已 publish 到 kellnr 不可删除；只能 yank（标记）它
- **Java SDK 错**：Reposilite 默认允许删除 release（看 UI 自己点）
- **npm 错**：内网 registry 同样允许 unpublish

### 4.3 已 deploy 到 staging，怎么回退 staging

```bash
ssh kudi@192.168.2.127
cd ~/coder-micro-services
# 找到上一次成功的 deploy 镜像 tag
git log --oneline --since='1 day ago'
# 把 STAGING_PROJECT_DIR 切回那次 commit
sudo git checkout <commit-hash> -- infrastructure/ services/ libs/ sdk/
# 然后跑：
sudo /home/kudi/coder-micro-services/infrastructure/scripts/deploy-remote.sh
```

或者直接：
```bash
# 重新触发 pipeline（把 fix/ci-e2e-ui-test-executor push 几个 revert commit 进来）
```

---

## 5. 紧急流程（CI 黄了但 prod 必须发版）

走"手动发布"路径：

```bash
# ops 手工跑
ssh kudi@192.168.2.127
cd ~/coder-micro-services
git pull

# 跑对应 publish step（不需要 CI）：
node packages/cli/bin/coder.js release publish maven --version 1.3.0
node packages/cli/bin/coder.js release publish rust --targets libs 1.3.0 --registry kellnr
node packages/cli/bin/coder.js release publish rust 1.3.0 --registry kellnr
node packages/cli/bin/coder.js build images -v 1.3.0 --push
pnpm run packages:build && pnpm run packages:publish  # @digua/contracts + @digua/http，token 在 ops 本地 ~/.npmrc
```

---

## 6. 实战示例：从 0 到 v1.3.0 发布

```bash
# 1. 切到 main，pull 最新
git checkout main && git pull origin main

# 2. 确认 main 上 commit 全绿（CI 没黄）
# 浏览器到 http://192.168.2.16/template/low-memory-micro-services/-/pipelines?ref=main

# 3. 看一眼当前 main HEAD 的 release notes 需要
git log --oneline v1.2.0..HEAD  # 看从上个 tag 累积了多少 commit

# 4. 打 tag 推送
git tag v1.3.0
git push origin v1.3.0

# 5. 监控（5-15 min 整条流水线跑完）
open http://192.168.2.16/template/low-memory-micro-services/-/pipelines?scope=tags

# 6. 验证下游 artifact 出现
curl -s http://mvn.kudi.repos/api/browse/io/digua/ | jq .  # 看到 v1.3.0
# 用 cargo search / npm view 之类验证

# 7. 写入 announcement
echo "v1.3.0 published; changelog: ..."
```

---

## 7. 已知边界

- **不能并发打 tag**：同一 repo 同时推 2 个 tag 会跑 2 条 pipeline，可能在 maven / kellnr 出现 "version already exists"。一次一个 tag。
- **kellnr cargo publish 不可删除已发布版本**（默认行为；需要 admin 后台 yank）
- **main 上合并 PR 不会被自动打 tag** —— 必须人工 `git tag && git push`

# 12. 发布与分发

> low-memory-micro-services 项目如何发布成可分发的制品，以及下游客户拿到制品后如何在它之上创建新的业务微服务。

---

## 12.1 概述：三种发布形态

| 形态 | 产物 | 适用场景 | 触发命令 |
|------|------|----------|----------|
| **A. 镜像推内网 registry** | 9 个 Docker 镜像（8 核心服务 + frontend），打 `:<version>` + `:latest` tag，推到 `zhcoder-docker-registry.com:8000/micro-server/*` | 内网有 registry、能联网拉镜像的环境 | `pnpm docker:build -- -v <ver> --push` |
| **B. 离线分发包（air-gap tar.gz）** | `coder-micro-services-${VERSION}-airgap.tar.gz`（约 10–15 GB）+ 同步 libs/sdk/contracts/skills/scripts/services 骨架 | 客户环境无网络 / 涉密 / 私有化部署 | `node release/dist-template/scripts/release.js v1.0.0` |
| **C. SSH 直接部署** | `docker compose down && pull && up` 到 192.168.2.128 | 单一固定生产主机（**已知路径错**，[风险] 见 §12.4） | 由 `.gitlab-ci.yml` 的 `deploy` job 自动触发 |

> 大多数场景用 **形态 B**（私有化）或 **形态 A**（内网微服务）。形态 C 是早期遗留，建议迁移到形态 A。

---

## 12.2 形态 A：推镜像到内网 registry

### 命令

```bash
# 构建全部 9 个服务镜像并推送到内网 registry
pnpm docker:build -- -v 1.0.0 --push
# = node scripts/build-docker.js -v 1.0.0 --push

# 仅构建单个服务
pnpm docker:build -- api-gateway -v 1.0.0 --push

# 本地留镜像 + 推送（单架构）
pnpm docker:build -- --load-and-push -v 1.0.0

# 多架构
pnpm docker:build -- --platform linux/amd64,linux/arm64 --push
```

底层走 `scripts/build-docker.js`，集成构建 + 打 tag + 推送一体化流程，覆盖 9 个镜像（api-gateway / token-service / iam-service / notification-service / audit-service / config-service / file-service / prue-admin-backend / frontend）。

### tag 命名

每个镜像同时打两个 tag：
- `zhcoder-docker-registry.com:8000/micro-server/<service>:<version>` — 精确回溯
- `zhcoder-docker-registry.com:8000/micro-server/<service>:latest` — 默认拉取

版本号通过 `-v <version>` 参数指定。

### CI 触发

`.gitlab-ci.yml` 的 `build_api_gateway` / `build_token_service` 等 job（line 132-148）使用 `build-docker.js` 构建并推送对应服务镜像。

> ⚠️ **历史遗留**：`build_credential_login_service` / `build_user_service` job 已废弃（ai-context §5.1 [风险]），但 CI 仍在跑。如不再维护，删 job 即可。

---

## 12.3 形态 B：离线分发包

### 命令

```bash
# 在 dev 仓库根目录，有网环境（建议 16GB+ 磁盘）
node release/dist-template/scripts/release.js v1.0.0

# 输出：
#   release/coder-micro-services-v1.0.0-airgap.tar.gz   ← 交付给客户
#   release/coder-micro-services-v1.0.0/                ← 同上解压版（供本机预览）
```

> **路径注意**：脚本位于 `release/dist-template/scripts/release.js`（早期 shell 版 `release.sh` 已迁 Node 删除）。别用缺了 `release/dist-template/` 前缀的写法；打包在 dev 仓库**根目录**跑 `node release/dist-template/scripts/release.js`。

### 脚本做了什么

按顺序：

1. `docker pull` 9 个核心服务镜像（registry 默认 `zhcoder-docker-registry.com:8000`，可用 `REGISTRY` 环境变量覆盖）
2. `docker save` 每镜像成 tar，放进 `${DIST_DIR}/images/`
3. 同步开发骨架（**从 dev 仓库根**复制到 dist 包）：
   - `libs/`（Rust 共享库）
   - `sdk/`（Java + Rust SDK）
   - `contracts/`（错误码 / 权限码 / 事件 / 约定）
   - `scripts/`（CI 门禁脚本）
   - `.agents/skills/` + `.agents/rules/`（**保留点开头，详见 §12.8**）
   - `services/` 骨架（空目录 + `agents-template.md` 模板）
   - `Cargo.toml`（剥掉 services/\* members，业务服务动态加入）
   - `pnpm-workspace.yaml` + `package.json`（根调度脚本）
   - `AGENTS.md`（协作约定）
4. `tar -czf coder-micro-services-${VERSION}-airgap.tar.gz` 打总包

### 关键选项（`node release/dist-template/scripts/release.js`）

| 命令 | 行为 |
|------|------|
| `node release/dist-template/scripts/release.js v1.0.0` | 默认：pull + save 全部镜像 + 同步骨架 |
| `node release/dist-template/scripts/release.js v1.0.0 --no-images` | 跳过 docker pull/save，仅打骨架包（供已有镜像的升级包） |
| `EXPORT_IMAGES=0 node release/dist-template/scripts/release.js v1.0.0` | 同上（环境变量风格） |
| `node release/dist-template/scripts/release.js --images v1.0.0` | 显式导出镜像（与默认等价，便于 CI 声明） |
| `node release/dist-template/scripts/release.js --help` | 打印完整用法 |

> 早期 shell 版 `release.sh` 已于 2026-08 迁 Node 删除（同一逻辑 = 现役 `release.js`）。以下"脚本做了什么/选项表格"按现役 JS 描述。
> `pnpm run release:build` = 上述 `release.js`（无参取默认版本）；单测 `pnpm run release:test`。

### CI 触发（**当前未自动化**）

`.gitlab-ci.yml` 的 `release:dist` job（line 216-241）tag 触发（`v\d+\.\d+\.\d+$`），但**只跑 `node bin/build-frontend.js` + 把 dist-template 当 artifact 上传**——**不自动调 `release.js` 产出 airgap tar.gz**。job 内注释（line 232-233）明示：「实际 release 由运维在有网环境执行」。

**改进方向**：在 `release:dist` job 的 `script:` 末尾追加 `node release/dist-template/scripts/release.js $CI_COMMIT_TAG`，artifact 同时上传 `release/coder-micro-services-${CI_COMMIT_TAG}-airgap.tar.gz`，即可 tag → 自动出包。改前需在 GitLab Runner 上验证 `docker save` 磁盘空间。

---

## 12.4 形态 C：SSH 部署到固定主机 [风险]

`.gitlab-ci.yml` 的 `deploy` job（line 150-165）走 SSH 到 `192.168.2.128`，执行 `docker-compose down → pull → up`。**ai-context §5.2 [风险]**：

1. 脚本内 `cd /opt/coder-micro-services` + `docker-compose -f infrastructure/docker-compose.yml ...`，但**仓库根不存在** `docker-compose.yml`（已迁到 `infrastructure/`，且新版用 `docker compose` 而非 `docker-compose`）——部署会立即失败
2. `down` → `up` 中间存在**非零停机窗口**
3. 生产密码 `DEPLOY_PASSWORD: Ewq321#@!` 明文存在 GitLab CI Variables — **必须立即轮换**

**建议**：形态 C 仅作历史参考。生产部署统一用形态 A（推 registry + 主机 pull）。

---

## 12.5 离线分发包结构（dist 包 vs dev 仓库）

拿到 tar.gz 解压后，**dist 包 ≠ dev 仓库的子集**。两者关键差异：

| 维度 | dev 仓库 | dist 包 |
|------|----------|---------|
| Compose 路径 | `infrastructure/docker-compose-services.yml` | `infrastructure/docker-compose-services.yml`（**与 dev 一致**，release.js 经 `release-transforms.js remap-volume` 重写 volume 挂载源路径） |
| 主入口 | `infrastructure/docker-compose.yml`（include 子文件） | `infrastructure/docker-compose.yml`（**与 dev 一致**，`bin/*.sh` 用 `-f infrastructure/docker-compose.yml` 调用） |
| Skill / rules 路径 | `.agents/skills/` + `.agents/rules/` | `.agents/skills/` + `.agents/rules/`（**与 dev 一致，保留点开头**；release.js 拷 `.agents/rules`，缺时回退 `.trae/rules`） |
| `services/` | 含 8 个核心服务 + agents-template.md | **空目录** + `agents-template.md`（业务服务动态创建） |
| `Cargo.toml` members | `services/*` 全列 | `services/*` 被 `sed` 剥掉，由 skill 创建时动态加入 |
| API Gateway 路由 | `infrastructure/config/api-gateway/routes/*.router.toml` | `infrastructure/config/api-gateway/routes/*.router.toml`（**与 dev 一致**） |
| 服务配置文件 | `services/<svc>/config/`（开发仓库源码内） | `infrastructure/config/<slug>/`（内聚；iam / notification / token 三服务首启自动 seed 或运行时读） |
| 运维入口 | pnpm 脚本 | Makefile（`make up` / `make verify` / ...） |
| `bin/` | 无（运维在 `infrastructure/`） | 12 个一键脚本（`start.sh` / `stop.sh` / `load-images.sh` / `generate-secrets.sh` / `upgrade.sh` 等） |
| `secrets/` | 无（token-service 启动时自举生成 RSA key pair 写入 `/app/keys/`） | 运行时由 `make secrets` 生成的密钥目录 |
| `MANIFEST.json` | 无 | 构建时生成（version / git_sha / built_at / images 清单） |

**dist 包不需要**：`services/` 下的核心服务源码（已编译进镜像）、`.gitlab-ci.yml`（无 CI）、`infrastructure/` 下 dev 用 overlay。

### 服务配置 seed 机制（iam / notification / token）

iam-service / notification-service / token-service 的 yaml 配置**不在 dev 源码树上**，分发包通过 `infrastructure/config/<slug>/` 挂载：

| 服务 | 分发包挂载 | 启动行为 |
|------|-----------|---------|
| iam-service | `./config/iam:/etc/iam-service` | 首启若 `config.yaml` 不存在 → 从二进制内嵌 `config.default.yaml` seed 出一份（`create_new` 原子写，与 iam-outbox-publisher 并发安全） |
| iam-outbox-publisher | 同上 | 共享同一份 seeded yaml |
| notification-service | `./config/notification:/etc/notification-service` | 同 iam |
| token-service | `./config/token:/app/config:ro` | 镜像 baked `config.yaml`（含 dev `database.url: ""`），启动时 `TOKEN_DATABASE_URL` env 覆盖 + fail-fast 校验 |

**运维改配置**：直接编辑 `infrastructure/config/<slug>/config.yaml`（iam/notification 首启后自动出现），重启容器生效；env 永远覆盖 yaml。

**配置字段参考**：见 [`docs-site/guides/config-model.md`](../guides/config-model.md)（三段瀑布总览）与各 `*-env.md`。

---

## 12.6 消费方快速开始（拿到 tar.gz 后 4 步）

```bash
# 1. 解压
tar -xzf coder-micro-services-v1.0.0-airgap.tar.gz
cd coder-micro-services-v1.0.0

# 2. 加载镜像（仅离线 / air-gap 环境需要；在线环境跳过）
node bin/load-images.js      # = make load-images

# 3. 生成密钥 + 改 .env
make secrets                 # = node bin/generate-secrets.js
# 把 .env 里的 __CHANGE_ME__ 占位一一替换为实际值（DB 密码 / JWT issuer / 域名等）

# 4. 启动 + 冒烟
make up                      # = node bin/start.js
make verify                  # 端到端冒烟测试
```

`make help` 看全部 12 个 make 命令。

> **重新配置镜像来源**：dist 包自带 `scripts/setup-wizard.js`（由 release.js 从 dev 仓库同步）。若需切换 Docker registry 来源（如从 Docker Hub 切到内网 mirror），跑：
> ```bash
> pnpm install && pnpm run setup:wizard   # 交互式配置
> pnpm run images:sync                    # 重新生成 base-images.json
> ```
> 详见 [Docker Registry 来源配置指南](../guides/docker-registry-config.md)。

访问地址（默认端口，dist 包内文档同）：

| 服务 | 地址 | 默认账号 |
|------|------|---------|
| 前端 | http://localhost:3000 | — |
| API 网关 | http://localhost:8080 | — |
| Consul | http://localhost:8500 | — |
| Prometheus | http://localhost:9090 | — |
| Grafana | http://localhost:3001 | admin / admin |
| Jaeger | http://localhost:16686 | — |
| RabbitMQ | http://localhost:15672 | guest / guest |

> 📌 **后续升级（v1.0 → v1.1 → v2.0 …）请用 [`cms-sync`](../guides/upstream-sync.md)** 自动同步配置文件 / 脚本 / contracts 的改动，而**不**是手动 diff tar.gz。本节是**首次**初始化流程；版本间升级见 §12.10。

---

## 12.7 在 dist 包上创建业务微服务（端到端 7 步）

dist 包的 `services/` 是空的。客户创建第一个业务微服务的完整流程：

```
1.  启动平台                            # make up + make verify（§12.6）
2.  AI 触发 skill                         # 对 AI 说："新增 order-service"（skills/rules 已在 .agents/，无需手动修正，§12.8）
3.  skill 自动注册                         # Cargo.toml + compose/services.yml + api-gateway routes
4.  编写业务代码 + 迁移                    # 按 skill 给出的 10 文件骨架填充
5.  跑 4 道 CI 门禁                        # §12.9
6.  构建并启动业务服务                     # docker compose -f docker-compose.yml up -d order-service
7.  verify 业务服务可访问                  # curl http://localhost:8080/v1/order/...
```

### 第 3 步的 skill 选择

| 业务服务栈 | Skill | 产出 |
|-----------|-------|------|
| **Rust**（推荐，路径最成熟） | `.agents/skills/coder-new-service/SKILL.md` | 10 文件最小集（main.rs / lib.rs / config.rs / openapi.rs / routes/ ...）+ 3 步自动注册 |
| **Java**（Spring Boot 3 + JDK 21） | `.agents/skills/coder-new-java-service/SKILL.md` | 多模块 Maven：`<svc>-bootstrap` / `<svc>-api` / `<svc>-domain` / `<svc>-infrastructure` |

加路由 / 加迁移：

| 场景 | Rust skill | Java skill |
|------|-----------|-----------|
| 加新 REST 端点 | `.agents/skills/coder-add-route/SKILL.md` | `.agents/skills/coder-add-java-route/SKILL.md` |
| 加新数据库迁移（双版本：MySQL + SQLite） | `.agents/skills/coder-migrate-db/SKILL.md` | `.agents/skills/coder-migrate-java-db/SKILL.md` |

**完整业务接入指南**（决策树 / 命名约定 / API Gateway 接入 / 服务发现 / 可观测性 / 11 步上线 checklist / 12 个常见坑）见 [`docs-site/guides/add-new-service`](../guides/add-new-service.md)，本文不重复。

---

## 12.8 dist 包的 skills / rules 路径（现值，无需改动）

`release.js` 打包时把 dev 仓库的 `.agents/skills/` 与 `.agents/rules/` **原样拷进 dist 包（保留点开头）**；`.agents/rules/` 缺失时回退 `.trae/rules/`。因此 dist 包里 AI 工具的 skill / rules 发现路径与 dev 仓库一致，**拿到即用、无需手动 symlink**。

> 历史：早期 shell 版 `release.sh` 曾把 skills 去点拷成 `skills/`，导致 AI 工具默认 `.agents/skills/` 找不到、需 `ln -s skills .agents/skills`。该问题随迁 Node 消失——现役 `release.js` 保留 `.agents/` 前缀。

**自检**（确认两目录都在、AI 工具能发现）：
```bash
ls .agents/skills/   # 应含全部 skill 的 SKILL.md
ls .agents/rules/    # auth-rules.md / project_rules.md / ...
```

### agents-template.md 在 dist 包内

`release.js` 在 `${REPO_ROOT}/services/agents-template.md` 存在时将其拷入 dist 包的 `services/agents-template.md`（`coder-new-service` skill 用该文件作为新服务的 AGENTS.md 模板）。假如缺失，skill 会报错：
```bash
ls services/agents-template.md   # dist 包内应存在
```
若缺失：回到 dev 仓库确认该文件存在，再重新跑 `release.js`。

---

## 12.9 CI 门禁（业务服务上线前必过）

dist 包内 `scripts/` 同步了 4 个 CI 门禁脚本。业务服务创建后必须全部通过：

```bash
node scripts/check-service-standard.js   # 8-item 服务规范（Dockerfile/AGENTS/README/tests/health/metrics/migrations/Swagger）
node scripts/check-db-naming.js          # 表名必须带服务前缀（iam_/audit_/token_/...）
node scripts/check-spec.js               # OpenAPI / 错误码 / 权限码 完整性
node scripts/contracts-gen.js --check    # 契约生成代码（libs/digua_common-error/src/codes.rs）与 YAML 一致
```

8-item 服务规范定义见 [`docs-site/decisions/ADR-002-service-standard-baseline`](../decisions/ADR-002-service-standard-baseline.md)。

---

## 12.10 升级流程

### ⚠️ 区分两层升级

| 升级对象 | 命令 | 频率 | 失败影响 |
|----------|------|------|----------|
| **配置 / 脚本 / contracts**（air-gap tarball 中的文件） | `npx cms-sync ./dist.tgz` | 每版必做 | 业务配置漂移、CI 脚本失效 |
| **Docker 镜像** | `make upgrade TAG=v1.x.y` | 每版必做 | 服务未运行新版本 |
| **数据 schema** | 容器启动自动跑 migration | 主版本升级时必检 | 数据不一致 |

**两层都要做**——`cms-sync` 只同步配置文件，**不**换镜像；`make upgrade` 只换镜像，**不**改配置文件。

### 12.10.1 配置文件升级：cms-sync（推荐）

> 完整工具文档：[`docs-site/guides/upstream-sync`](../guides/upstream-sync.md)。

```bash
# 1. 拿到新版本 air-gap tarball
# 例如 coder-micro-services-v1.1.0-airgap.tar.gz

# 2. 干跑 — 看会发生什么，不修改文件
npx cms-sync --dry-run ./coder-micro-services-v1.1.0-airgap.tar.gz

# 3. 实际同步
npx cms-sync ./coder-micro-services-v1.1.0-airgap.tar.gz
# 退出码 0 = 成功；2 = 有冲突需手动解决

# 4. 解决冲突（如有）：编辑冲突文件，删除 <<<<<<< 标记
#    冲突文件的 base 版本在 <file>.merge-base，方便对比

# 5. 跑 CI 门禁
pnpm check && pnpm test
node scripts/check-service-standard.js
node scripts/contracts-gen.js --check

# 6. 提交
git add -A && git commit -m "chore(sync): upstream v1.0.0 → v1.1.0"
```

**首次使用前**需要在项目根目录创建 `.upstream-sync.yaml`（声明哪些文件 managed / three-way-merge / skip）和 `.upstream.lock`（指向 v1.0.0 初始 commit），详见 [`docs-site/guides/upstream-sync`](../guides/upstream-sync.md) §3 / §4.1。

**回滚**：`cms-sync` 写盘前可以走 `git reset --hard HEAD`（前提：sync 前已 commit 当前状态）。

### 12.10.2 镜像升级：make upgrade

`make upgrade` 只换 Docker 镜像，**不**改配置文件：

```bash
# 在线（能 docker pull 的环境）
make upgrade TAG=v1.1.0
# 等价：node bin/upgrade.js v1.1.0

# 离线（air-gap 环境，先加载新版本镜像）
node bin/load-images.js
make upgrade TAG=v1.1.0
```

`upgrade.sh` 行为：拉新镜像（覆盖 `:latest` tag）→ `docker compose pull` → `docker compose up -d` → `make verify`。

### 12.10.3 ⚠️ 跨版本 schema 兼容性

`make upgrade` 仅做镜像替换，**不自动执行数据库迁移**。各服务的 schema migration（Rust 用 `sqlx migrate run`、Java 用 Flyway）在容器启动时自动跑，但**跨主版本升级（v1.0 → v2.0）可能有不兼容的 schema 变更**，需：

1. 升级前先看 `release/dist-template/CHANGELOG.md` 或本次发布的 release notes
2. 若有 breaking change，先在测试环境跑 `make upgrade`，看 `make verify` 是否通过
3. 数据备份：`mysqldump` 全库 + `redis-cli BGSAVE`
4. 失败回滚：`make reset-data` 会清空所有数据（**慎用**），需用备份恢复

### 12.10.4 标准升级组合

```bash
# 1. 同步工具自身升级（如果新版）
pnpm update @digua/dist-tools

# 2. 同步上游配置 / 脚本 / contracts
npx cms-sync ./coder-micro-services-v1.1.0-airgap.tar.gz
# 解决冲突（如有）

# 3. 升级镜像
make upgrade TAG=v1.1.0

# 4. 跑全量 CI 门禁
pnpm check && pnpm test
node scripts/check-service-standard.js
node scripts/contracts-gen.js --check

# 5. 提交
git add -A && git commit -m "chore(sync): upstream v1.0.0 → v1.1.0"
```

---

## 12.11 发布检查清单

### Dev 仓库维护者（打 release 前）

- [ ] `main` 分支已合并本期所有 PR，`git status` 干净
- [ ] `pnpm check && pnpm test && pnpm build` 全过
- [ ] CI `test` 阶段全绿
- [ ] `release/dist-template/scripts/release.js` 同步本期新增的 `services/<new-svc>` 到 dist 包（如有）
- [ ] `release/dist-template/CHANGELOG.md` 更新本期变更（手动维护，暂无自动同步）
- [ ] 打 tag：`git tag v1.x.y && git push origin v1.x.y`

### SDK 发布（与平台 tag 解耦，独立节奏）

SDK 是给业务方集成用的，与平台 release tag **解耦**。三类 SDK 各有发布脚本，**VERSION 必填**：

| SDK | 目标 | 发布命令 | 详细 |
| --- | --- | --- | --- |
| Java SDK (sdk/java) | Reposilite (`http://mvn.kudi.repos`) | `node scripts/publish-maven-release.js --version <ver>` | [java-sdk-publishing](../guides/java-sdk-publishing.md) |
| Java SDK snapshot | Reposilite snapshots | `node scripts/publish-maven-snapshot.js --version <ver>-SNAPSHOT` | 同上 |
| Rust libs (libs/*) | kellnr (`http://192.168.2.12:16444`) | `node scripts/publish-libs.js <ver>` | [rust-sdk-publishing §8](../guides/rust-sdk-publishing.md)（与 SDK 共享同一份版本号） |
| Rust SDK (sdk/rust/*) | kellnr | `node scripts/publish-rust-sdks.js <ver>` | [rust-sdk-publishing](../guides/rust-sdk-publishing.md) |

等价 pnpm aliases：`pnpm sdk:publish:release -- <ver>` / `pnpm sdk:publish:snapshot -- <ver>` / `pnpm sdk:publish:libs -- <ver>` / `pnpm sdk:publish:rust -- <ver>`。

**所有发布脚本在 deploy 前会：**
- Java：自动 `mvn versions:set` 把 `sdk/java/pom.xml` 的 `<version>` 改成 `<ver>`（**不自动 git commit**，需手动 `git commit -m "feat(sdk): bump to <ver>"`）
- Rust：自动把 17 / 7 个 crate 的 `version = "..."` 改成 `<ver>`（脚本处理完会自动恢复任何备份）

发 SDK 前 checklist：

- [ ] `pnpm run setup:wizard` 已生成 maven/kellnr 凭证（或手写 `~/.m2/settings.xml` / `~/.cargo/credentials.toml`）
- [ ] `node scripts/publish-libs.js <ver> --dry-run` 退出 0
- [ ] `node scripts/publish-rust-sdks.js <ver> --dry-run` 退出 0
- [ ] `node scripts/publish-maven-release.js --version <ver> --dry-run` 退出 0
- [ ] 真发布；Java SDK 发完后手动 `git diff sdk/java/ && git add sdk/java && git commit -m "feat(sdk): bump to <ver>"`

> CI 自动跑：`.gitlab-ci.yml` 的 `sdk_publish` / `rust_publish` job 在 `v\d+\.\d+\.\d+$` tag 触发时自动注入 `${CI_COMMIT_TAG#v}` 作为 VERSION。本地 tag 触发也会跑。

### 运维（打包）

- [ ] 在有网环境，磁盘 ≥ 20 GB 可用
- [ ] `node release/dist-template/scripts/release.js v1.x.y` 执行成功
- [ ] `release/coder-micro-services-v1.x.y-airgap.tar.gz` 文件存在
- [ ] `sha256sum` 记录归档（**建议**：CI 自动跑，artifact 一并上传）

### 客户（消费）

- [ ] 解压后 `ls -L .agents/skills/ .agents/rules/ services/agents-template.md` 等关键文件可读
- [ ] skills / rules 无需改路径——已在 `.agents/` 前缀下（§12.8）
- [ ] `make secrets` 生成的 `.env` 已填入实际值（`__CHANGE_ME__` 全替换）
- [ ] `make up && make verify` 通过
- [ ] 创建第一个业务服务后跑通 §12.9 全部 4 道 CI 门禁

---

## 12.12 故障排查

| 现象 | 原因 | 解决 |
|------|------|------|
| 打包命令找不到 / 报未知参数 | 用错路径或把 git tag 的 `v1.5.0` 当镜像 tag（registry 里是 no-v `1.5.0`） | 用完整路径 `node release/dist-template/scripts/release.js <git-tag>`；离线引用/下游 `IMAGE_TAG` 用 no-v（§12.3） |
| `release:dist` CI job 没产出 tar.gz | 该 job 只 build frontend，**不调** `release.js` | 手动跑 `node release/dist-template/scripts/release.js $CI_COMMIT_TAG`，或在 CI job `script:` 末尾追加该命令（§12.3） |
| `coder-new-service` 报"找不到 agents-template.md" | dist 包内缺该模板 | 回到 dev 仓库确认 `services/agents-template.md` 存在，重跑 `release.js` |
| 业务服务 `docker compose up` 找不到 service | skill 把服务加到了错误的 compose 路径 | dev 仓库内 skill 改 `infrastructure/docker-compose-services.yml`；dist 包内要改 `compose/services.yml` —— 当前 skill **硬编码** dev 仓库路径，在 dist 包内**跑 skill 不会自动注册到 compose**，需手动复制 service entry（已知缺口，见 ai-context §5 待补） |
| `make verify` 失败 | 镜像未启动 / 端口冲突 / Consul 未发现 | `docker compose ps` 看 healthcheck；`make logs` 看启动日志；`curl http://localhost:8500/v1/catalog/services` 看注册情况 |
| `make upgrade` 后业务服务连接断开 | 容器 IP 变了，env 缓存了旧地址 | `make restart <svc>` 重启业务服务；长期方案：业务服务全部走 Consul DNS / env 注入 |
| `npx cms-sync` 报 `Cannot find module 'tar'` | `@digua/dist-tools` 没装 / `node_modules` 缺失 | `pnpm install` 或 `pnpm add -D @digua/dist-tools` |
| `npx cms-sync` 报 `no previous sync recorded` | 首次 sync 或 `.upstream.lock` 被 `.gitignore` 误屏蔽 | 首次属正常；若非首次，检查 `.upstream.lock` 是否被 git 跟踪 |
| `npx cms-sync` 报 `no .upstream-sync.yaml found` | 项目根目录缺策略文件 | 按 [`guides/upstream-sync`](../guides/upstream-sync.md) §3 创建 |
| `npx cms-sync` 退出码 = 2 留下 CONFLICT 标记 | 上游和下游改了同区域 | 编辑文件解决冲突，删 `<<<<<<<` 标记，删除 `<file>.merge-base`；详见 [`guides/upstream-sync`](../guides/upstream-sync.md) §6 |
# Infrastructure — Docker Workflow

> 完整 docker compose + pnpm 脚本速查。新人 onboarding 入口；日常 dev 参考。
>
> 本文只覆盖本地 dev 的容器编排与脚本别名。**Kubernetes / 生产部署**走 GitLab CI，不在本目录范围内。`infrastructure/AGENTS.md` 记录本目录独立的 loop 约定（test / lint / L1→L2），与本 README 互不冲突。

## Compose 文件清单

5 个文件分层叠加，避免重复定义。修改时请明确改的是哪一层。

| 文件 | 角色 | 何时使用 |
| --- | --- | --- |
| `docker-compose.yml` | 主入口 | 唯一直接 `up` 的文件；`include` 下面 3 个。 |
| `docker-compose-infra.yml` | 基础设施基线 | MySQL / Redis / Consul / RabbitMQ / registrator。无端口发布、无 dev 覆盖。 |
| `docker-compose-infra.dev.yml` | dev overlay | 给基础设施加端口发布（3306 / 6379 / 8500 / 5672 等），方便宿主机直连。 |
| `docker-compose-services.yml` | 业务服务 | 8 个 Rust 服务 + 2 个 outbox publisher 的定义与依赖关系（frontend 服务已随 PRISM console 移除）。 |
| `docker-compose-observability.yml` | 可观测性 overlay | Prometheus / Grafana / Jaeger / OTel Collector / consul-template；已 `include` 进主入口。 |

> ⚠️ 不要再写第 6 个 compose 文件做「dev-only 全栈」——dev/prod 差异统一通过 `_dev` overlay 表达。

### MySQL 主从复制

`docker-compose-infra.yml` 内置了 MySQL 一主一从：

| 服务 | 角色 | 端口 | 说明 |
| --- | --- | --- | --- |
| `mysql` | 主库（primary） | 3306 | 读写，binlog + GTID 开启 |
| `mysql-replica` | 从库（replica） | —（仅 expose，不对外发布） | 只读热备，GTID 自动同步 |

**从库作用**：
- **灾备 failover**：主库宕机时 promote 从库为新主库，切换业务连接串后恢复服务
- **读写分离预留**：2.0 规划将只读查询路由到从库，分担主库压力

**从库镜像**：本地 dev 走 `docker compose build` 从 `infrastructure/mysql-replica/Dockerfile` 构建（compose 里同时声明了 `build:` 与 `image: coder/mysql-replica:8.0` 作为本地 tag）；CI 由 `.gitlab-ci.yml` 的 `build_images:staging` 单独 buildx --push 到 `zhcoder-docker-registry.com:8000/micro-server/mysql-replica:<sha>`，`release:staging` 阶段 `transform-compose` 把 compose 的 `build:` 块替换为该 registry image 引用。

entrypoint 启动时自动等主库可达 → `CHANGE MASTER TO` → `START SLAVE`，无需手动 dump/restore。

```bash
# 启动从库（自动同步）
docker compose -f infrastructure/docker-compose.yml up -d mysql-replica

# 查看同步状态
docker compose -f infrastructure/docker-compose.yml exec mysql-replica \
  mysql -u root -p"${MYSQL_ROOT_PASSWORD}" \
  -e "SHOW SLAVE STATUS\G" | grep -E "Running|Behind_Master"
```

> 完整运维 SOP（监控、故障处理、failover 步骤）见 [`docs-site/runbooks/mysql-replication.md`](../docs-site/runbooks/mysql-replication.md)。

## 前置检查

任何 docker 操作之前跑一次：

```bash
pnpm check:env
```

会校验 docker daemon、`infrastructure/.env` 完整性、端口占用、必要目录等。失败时脚本会给出明确报错，不要绕过。

## pnpm 脚本速查表

> 全部从仓库根目录运行。`docker compose -f infrastructure/docker-compose.yml` 在 package.json 里是固定前缀，省得每次手写路径。

### 首次启动

| 命令 | 做什么 |
| --- | --- |
| `pnpm run setup` | 全量初始化（`pnpm install` + 镜像同步 + swagger-ui 离线包 + `check:env`）。CI 镜像 / 全新 clone 后跑一次。 |
| `pnpm run dev:setup` | 一站式首次启动：env 检查 → 构建镜像 → `up -d` → 灌 demo 数据 → 打印登录账号清单。 |
| `pnpm run check:env` | 仅做环境体检，不起容器。 |

### 日常操作

| 命令 | 做什么 |
| --- | --- |
| `pnpm run dev:infra` | 只起基础设施（Consul / Redis / MySQL / RabbitMQ）。服务进程跑宿主机时用这个。 |
| `pnpm run dev:infra:e2e` | 同上，**额外叠加** `docker-compose-infra.dev.yml` 把 3306 等端口发布到宿主机。本机跑 `tests/e2e`（Vitest + supertest 直连 mysql 重置库）时**必须用这个**。 |
| `pnpm run dev` | 起基础设施 + 各 workspace 包的 `dev` 任务并行（含 `packages/` TS 包构建等）。 |
| `pnpm run dev:ps` | `docker compose ps`，看容器状态与 healthcheck。 |
| `pnpm run dev:logs` | tail 所有容器日志。退出用 Ctrl-C。 |
| `pnpm run dev:restart` | 重启全部容器（不重建镜像）。代码改动后用。 |
| `pnpm run dev:down` | 停全部容器。**保留** named volumes（数据还在）。 |

### 构建镜像

| 命令 | 做什么 |
| --- | --- |
| `pnpm run docker:compose:build` | 构建所有服务的镜像（走 `docker compose build`）。 |
| `pnpm run docker:compose:build:<svc>` | 仅构建某一个服务，例如 `docker:compose:build:iam-service`。 |
| `pnpm run release:build` | 产出完整离线分发包（airgap）：镜像 tar + compose + dev 骨架。 |
| `pnpm run release:deploy` | 产出纯部署包（deploy）：镜像 tar + compose + 精简 ops 脚本（无 dev 工具）。 |
| `pnpm run release:deploy:test` | dry-run：`EXPORT_IMAGES=0` 跑 release-deploy.js，验证骨架结构（不实际 build/save 镜像）。 |
| `pnpm run release:all` | 一次性跑 release:build + release:deploy。 |
| `pnpm run docker:build` | 走 `coder build images`，构建所有镜像。无参数=交互式；参数式支持 `-v <ver>` / `--push` / `--load-and-push` / `--platform linux/amd64,linux/arm64`。详见 `node packages/cli/bin/coder.js build images --help`。 |
| `pnpm run docker:build:<svc>` | 单服务构建（= `node packages/cli/bin/coder.js build images <svc>`，参数式默认不推送）。 |
| `pnpm run docker:build:clean` | prune dangling images 释放磁盘。 |

常用参数式示例：

```bash
pnpm run docker:build -- -v 1.0.0 --push                              # 版本号 + 直推
pnpm run docker:build -- --load-and-push -v 1.0.0                     # 本地留镜像 + 推送
pnpm run docker:build -- --platform linux/amd64,linux/arm64 --push    # 多架构（自动 --push）
```

> 单服务名列表：`api-gateway` / `token-service` / `iam-service` / `notification-service` / `audit-service` / `config-service` / `file-service` / `prue_backend`（原 `frontend` 已随 PRISM console 移除）。`prue_backend` 在 `docker:compose:build` 写法里用 `_`（compose 服务名），在 `docker:build` 写法里用 `-`（脚本别名）。

### Demo 数据

| 命令 | 做什么 |
| --- | --- |
| `pnpm run docker:demo:data` | 追加 demo 用户 / 角色 / 权限，**不清空** 现有业务表。 |
| `pnpm run docker:demo:data -- --reset` | 先清空业务表再灌。可重复执行。 |

### 测试

| 命令 | 做什么 |
| --- | --- |
| `pnpm run test:e2e` | 全量 e2e（Vitest + supertest，跨真服务）。 |
| `pnpm run test:e2e:<group>` | 按业务分组跑：`auth` / `users` / `tokens` / `roles` / `permissions` / `audit` / `config` / `file` / `notification` / `token-admin` / `menu-configs` / `iam-orgs` / `gateway`。 |
| `pnpm run test:e2e:api` | 数据驱动 API 用例（Excel 数据源）。 |
| `pnpm -C tests/e2e test:smoke` | 2 分钟 smoke 子集（仅 API 用例，直打 api-gateway:3000；原 Playwright UI 套件已随 PRISM console 移除删除）。 |
| `node scripts/run-tests-docker.js [svc...]` | 容器内跑 Rust `cargo test`，详见根 README「在 Docker 中跑 Rust 单元测试」一节。 |

## 端口映射

`docker compose up` 之后，宿主机可直接访问的端口：

| 容器 / 服务 | 外部端口 | 内部端口 | 用途 |
| --- | --- | --- | --- |
| consul | 8500 | 8500 | HTTP UI / 服务发现 |
| consul | 8502 | 8502 | gRPC |
| redis | 6379 | 6379 | 缓存 / token 黑名单 |
| mysql | 3306 | 3306 | 业务数据库（`MYSQL_PORT_EXPORT` 可改） |
| rabbitmq | 5672 | 5672 | AMQP |
| rabbitmq management | 15672 | 15672 | UI（guest / guest） |
| api-gateway | 3000 | 3000 | 对外 HTTP 入口（唯一公共入口） |
| token-service | 8082 | 8082 | JWKS / token 签发 |
| iam-service | 9081 | 8081 | 用户 / 角色 / 权限（端口错位避开 host 6002 冲突） |
| notification-service | 8083 | 8083 | 通知 / 邮件 / 短信 |
| audit-service | 8084 | 8084 | 审计日志 |
| config-service | 8085 | 8081 | 动态配置 |
| file-service | 8086 | 8082 | 对象存储 |
| prue_backend | 8087 | 8087 | 流程引擎（SQLite + MySQL 双栈） |
| prometheus | 9090 | 9090 | 指标查询 / Consul SD |
| grafana | 3001 | 3000 | Dashboard（admin / admin，首次登录后改） |
| jaeger | 16686 | 16686 | Trace UI |
| otel-collector | 4317 | 4317 | OTLP gRPC 接收 |
| otel-collector | 4318 | 4318 | OTLP HTTP 接收 |
| otel-collector | 8889 | 8889 | Prometheus exporter |

> 宿主机直接 `cargo run` 启动服务时默认端口是 6001-6007（见根 README「服务端口与依赖」表）。本文 8000+ 系列是 `docker compose` 内端口，**两套并存**：宿主机进程和容器互不干扰。

## 典型工作流

### 第一次跑通

```bash
pnpm check:env                    # 环境体检
pnpm run dev:setup                # 构建 + 起 + 灌 demo 数据 + 打印账号
docker compose -f infrastructure/docker-compose.yml ps   # 看 healthcheck
curl -fsS http://localhost:3000/health                  # 网关通？（api-gateway :3000 唯一公共入口）
```

### 日常开发循环

改完 Rust 代码：

```bash
pnpm run docker:compose:build:<svc>     # 重建该服务镜像
pnpm run dev:restart                    # 全部容器重启（新镜像生效）
pnpm run dev:logs | grep <svc>          # tail 看启动日志
```

改前端 TS 集成包（`packages/`，业务仓前端消费的 npm 包）：

```bash
pnpm run packages:build                  # 构建 @digua/contracts + @digua/http
pnpm run packages:check                  # 发布前校验
pnpm run packages:publish                # 发布到私有 registry npm.kudi.repos
```

只动配置 / SQL 迁移，不动二进制：

```bash
pnpm run dev:restart <svc>              # docker compose 原生支持服务名
```

### 重置数据

```bash
pnpm run docker:demo:data -- --reset    # 清业务表 + 灌 demo
# 或：完全重置（含基础设施 volumes）
docker compose -f infrastructure/docker-compose.yml down -v
pnpm run dev:infra
```

### 停止并清理

```bash
pnpm run dev:down                       # 停容器，保留 volumes
pnpm run docker:build:clean             # prune dangling 镜像
```

## Staging 持续部署（GitLab CI）

`.gitlab-ci.yml` 新增 **continuous staging flow**。merge to main 后自动跑：build → release → deploy → e2e，失败自动回滚。

### 关键设计

| 维度 | 选择 | 原因 |
| --- | --- | --- |
| 镜像 tag | **`$CI_COMMIT_SHORT_SHA`**（per-commit，不变） | 无 race；每个 commit 都有 immutable 版本；保留历史便于回滚 |
| 部署模式 | **atomic symlink swap** | `staging-active -> staging-<sha>` 唯一可变，其他不可变 |
| 并发控制 | `resource_group: staging` | GitLab 串行化所有 staging job（防御性，per-SHA 本就不 race）|
| 镜像传递 | **registry pull**（不用 tarball） | artifact ~1MB vs 10GB；省 GitLab artifact 配额 |
| 失败回滚 | `after_script` swap 回 `staging-prev` | e2e 失败 → 上一版本立刻恢复 |

### Staging server 目录结构（192.168.2.31）

```
/home/kudi/coder-micro-services/
├── staging-active -> staging-<current-sha>      # 原子软链，CI deploy 后指新版本
├── staging-prev   -> staging-<previous-sha>     # 上一版 active，e2e 失败时回滚锚点
├── staging-abc1234/                             # immutable per-SHA 目录
├── staging-def5678/                             # immutable per-SHA 目录
└── ...
```

每个 `staging-<sha>/` 是 self-contained 部署目录（来自 release-deploy.js 的产物 rsync 上去）。GC 由 `infrastructure/scripts/staging-gc.sh` 在 cron.daily 清理老目录（保留最近 5 个 + 永远保护 active/prev）。

### CI flow 4 个 job（详见 `.gitlab-ci.yml` 第 372 行起）

```
build_images:staging → release:staging → deploy:staging → e2e:staging
                                              ↓ (失败)
                                         swap 回 staging-prev
```

| Job | 阶段 | 关键命令 |
| --- | --- | --- |
| `build_images:staging` | build | `pnpm run docker:build -- -v $CI_COMMIT_SHORT_SHA --load-and-push ...`（8 服务 + mysql-replica 单独 buildx；frontend 已移除）|
| `release:staging` | build | `node release/dist-template/scripts/release-deploy.js $SHA --no-images`（产骨架）|
| `deploy:staging` | deploy | tar over ssh → `load-images.js --mode=pull` → `swap-staging-active.sh` |
| `e2e:staging` | e2e | SSH 反向隧道 + `pnpm run test:e2e:api`（UI 套件已移除）；失败 swap 回 prev |

### 手动操作（CI 不可用时 fallback）

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

## 故障排查

### MySQL 连接失败

- 容器没起来：`pnpm run dev:ps` 看 healthcheck；`docker logs <mysql>` 看启动日志。
- 端口被占：宿主 3306 被本机 MySQL 占 → 改 `infrastructure/.env` 的 `MYSQL_PORT_EXPORT`。
- 密码错误：默认 `rootpass` / `apppass`，改在 `.env` 重启容器生效。

### 容器端口冲突

外层端口（compose `ports` 字段左侧）冲突时，改 `infrastructure/.env` 对应的 `*_PORT_EXPORT` / `*_PORT` 变量，然后 `pnpm run dev:restart`。**不要**直接改 compose 文件，会被 git 跟踪。

### Demo 数据没灌进去

- `pnpm run check:env` 先确认 MySQL 可达。
- 看 `scripts/demo-data.js` 的 stderr：默认从 `infrastructure/.env` 读 MySQL 连接；3306 未发布时会自动 fallback 到 `docker exec`。
- 重复跑同一条命令会主键冲突；用 `--reset` 清表。

### 服务起不来

- 看 healthcheck：`pnpm run dev:ps` 状态列；`pnpm run dev:logs <svc>` tail。
- 镜像过期：`pnpm run docker:compose:build:<svc>` 重建。
- 依赖未满足：`docker:compose:build` 重新构建后 `dev:down` + `dev:infra` 冷启动。

## 相关文档

| 场景 | 文档 |
| --- | --- |
| 新人入职（环境 → 跑通 → 排查） | [`docs-site/onboarding/README.md`](../docs-site/onboarding/README.md) |
| 完整用户手册（中文，分章节） | [`docs-site/user-manual/README.md`](../docs-site/user-manual/README.md) |
| AI 协作入口（service map + pitfalls） | [`docs-site/ai-context.md`](../docs-site/ai-context.md) |
| 可观测性 stack（Prometheus / Grafana / OTel） | [`docs-site/guides/observability.md`](../docs-site/guides/observability.md) |
| OTel trace 接入 | [`docs-site/guides/otel-tracing.md`](../docs-site/guides/otel-tracing.md) |
| 灰度发布（canary tag + nginx） | [`docs-site/guides/canary-rollout.md`](../docs-site/guides/canary-rollout.md) |
| E2E 测试（smoke / 数据驱动） | [`docs-site/guides/testing.md`](../docs-site/guides/testing.md) |
| 加路由 / 迁移 / 新建 service | [`.agents/skills/`](../.agents/skills/) |
| 服务规范 + 强制规则 | [`.trae/rules/project_rules.md`](../.trae/rules/project_rules.md) |
| 架构决策记录（ADR） | [`docs-site/decisions/`](../docs-site/decisions/) |
| 当前 wrap-up 计划 | [`.omo/plans/docker-compose-dev-infra-wrapup.md`](../.omo/plans/docker-compose-dev-infra-wrapup.md) |
| 踩坑沉淀 | [`.omo/notepads/docker-compose-build-script/learnings.md`](../.omo/notepads/docker-compose-build-script/learnings.md) |

## 根 README 速链

- **服务端口与依赖**（宿主机 `cargo run` 的 6001-6007 端口）：见根 README §服务端口与依赖。
- **Swagger UI 地址表**：见根 README §Swagger UI。
- **限流配置 / 灰度发布**：见根 README §API Gateway 限流 / §灰度发布。
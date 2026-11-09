# 部署单元镜像引用 env 化（.image.env 驱动） — 设计规格

- **日期**：2026-11-09
- **状态**：设计已定稿，待实施
- **触发**：用户要求「docker-compose.yml 里的镜像名改成环境变量，定义在 `.image.env`；
  每次发布镜像后用 js 脚本更新 `.image.env`；`start.sh` 同时加载 `.image.env` 和 `.env`」
- **承接**：[2026-11-02-release-image-env-driven-design.md](2026-11-02-release-image-env-driven-design.md)
  （该 spec 做的是「官方镜像整名变量化」，本次做的是「**本系统自建镜像**整名变量化 +
  版本收敛到单一文件」）

---

## 1. 现状与问题

`examples/docker-deploy/`（services 组自包含部署单元）当前用**拼接型**变量引用镜像：

```yaml
image: "${kudi_repos}/file-service:${IMAGE_TAG_PG}"
```

问题（承自 2026-11-02 spec §2.1 的实测结论）：

| 问题 | 影响 |
| --- | --- |
| 拼接型变量未定义时 compose **只 warning**，静默展开成 `/file-service:1.7.0` | 错误推迟到 `docker pull` 才暴露 |
| 版本只有两个变量（`IMAGE_TAG_PG` / `IMAGE_TAG_GATEWAY`） | 无法逐服务独立钉版本（回滚单个服务做不到） |
| 镜像名硬拼在 compose 里 | 换 registry / 换服务镜像名要改 compose 文件，而非改配置 |

## 2. 目标

1. **本系统自建镜像的引用在 compose 中一律是整名变量**：`image: "${IMAGE_FILE_SERVICE}"`。
2. **版本信息只在 `.image.env` 一处**：发布镜像 → 跑一次脚本 → `.image.env` 更新 → 部署单元自带钉好的版本。
3. `.image.env` **入库**（无凭据），部署单元整目录拷到目标机即可复现同一套镜像版本。
4. `.env` 只管**环境配置**（IP / 口令 / 拓扑），不再混镜像版本。
5. `start.sh` 同时加载 `.image.env` 与 `.env`，缺变量时**启动前**报错，不留到 pull 阶段。

### 非目标

| 项 | 结论 | 理由 |
| --- | --- | --- |
| 官方镜像（redis / mysql / consul …） | **本次不动** | 已在 `infrastructure/infra-*/.env` 用整名变量（`redis=…`），属另一组部署单元 |
| `services/*/Dockerfile` 的 `FROM ${BASE_REGISTRY}/…` | 不改 | 构建期 base image 与运行时 image 是两套东西 |
| `packages/cli` 的镜像构建逻辑 | 不改 | 镜像 tag 规则（`<ver>` / `<ver>-pg`）沿用 `lib/docker-build.js` |

## 3. 变量清单

`.image.env`（7 个变量 ↔ 7 个业务服务镜像）：

| 变量 | compose service | registry 镜像名 | 方言 |
| --- | --- | --- | --- |
| `IMAGE_API_GATEWAY` | `api-gateway` | `api-gateway` | 无关方言 |
| `IMAGE_TOKEN_SERVICE` | `token-service` | `token-service` | 带 DB |
| `IMAGE_IAM_SERVICE` | `iam-service` | `iam-service` | 带 DB |
| `IMAGE_NOTIFICATION_SERVICE` | `notification-service` | `notification-service` | 带 DB |
| `IMAGE_FILE_SERVICE` | `file-service` | `file-service` | 带 DB |
| `IMAGE_AUDIT_SERVICE` | `audit-service` | `audit-service` | 带 DB |
| `IMAGE_PRUE_BACKEND` | `prue_backend` | `prue-admin-backend` | 带 DB |

- **命名**：`<IMAGE_> + 大写服务名`；compose service 名与 registry 镜像名不一致的只有
  `prue_backend` → `prue-admin-backend`（沿用 2026-11-02 spec §4.2 的特殊映射）。
- **outbox publisher 不单列变量**：`iam-outbox-publisher` / `notification-outbox-publisher`
  复用主服务镜像（compose 里直接引用同一个 `IMAGE_IAM_SERVICE` / `IMAGE_NOTIFICATION_SERVICE`）。
- **方言后缀**：`pg` 方言 → tag 加 `-pg`（如 `1.7.0-pg`）；`mysql` 方言 → 无后缀。
  api-gateway 无 DB，两种方言都无后缀。

## 4. 文件与加载顺序

```
examples/docker-deploy/
├── .image.env        # 镜像引用（入库，脚本生成）
├── .env              # 环境配置（不入库，含口令）
└── docker-compose-services.yml
```

`start.sh` 的 env 加载顺序（后者覆盖前者）：

1. `$DIR/../../.env` → `$DIR/../.env` → `$DIR/.env`（向上回退，仓内命中仓根）
2. `$DIR/../../.env.prod` → `$DIR/../.env.prod` → `$DIR/.env.prod`（运维覆盖层，可选）
3. `$DIR/../../.image.env` → `$DIR/../.image.env` → `$DIR/.image.env`（**镜像钉版，最后加载 → 权威**）

镜像钉版放最后：`.env` 里的运维配置**不能**意外改掉已发布的镜像版本。
同时 `docker compose` 调用显式带 `--env-file .image.env --env-file .env`，
使手工在目录内执行 `docker compose config` 也能复现同一份展开结果。

## 5. compose 写法

用 **`:?` 必填语法**，缺变量时 compose 立刻报出可操作的错误，而不是静默降级：

```yaml
image: "${IMAGE_FILE_SERVICE:?IMAGE_FILE_SERVICE 未定义——先加载 .image.env（node scripts/update-image-env.js -v <ver>）}"
```

实测（docker compose v5.6.1）：

```
$ docker compose -f docker-compose.yml config
error while interpolating services.a.image: required variable IMAGE_A is missing a value: ...
```

选 `:?` 而非 `:-` 兜底的理由：兜底值必须写死某个版本，会形成**第二真相源**并随发布变陈旧；
`:?` 让「版本只在 `.image.env`」这条不变量成立。

## 6. 更新脚本 `scripts/update-image-env.js`

```
node scripts/update-image-env.js -v 1.7.0 [--dialect pg|mysql] [--registry <host>] [--repo <prefix>]
                             [--dir examples/docker-deploy] [--check] [--verify] [--dry-run]
```

| 参数 | 语义 |
| --- | --- |
| `-v/--version` | 版本号；**自动剥前导 `v`**（CI 推送时用 `${CI_COMMIT_TAG#v}`，registry 里没有 `v`） |
| `--dialect` | `pg`（默认）/ `mysql`，决定 tag 后缀 |
| `--registry` / `--repo` | 默认 `zhcoder-docker-registry.com:8000` / `micro-server` |
| `--dir` | 部署单元目录，默认 `examples/docker-deploy` |
| `--check` | 只比对「期望内容 vs 现文件」，有漂移则 exit 1（可作 CI 门禁） |
| `--verify` | 调 registry v2 API 逐个确认镜像存在（Bearer 认证），缺失则 exit 1 |
| `--dry-run` | 打印将写入的内容，不落盘 |

写入策略：**整体重写** `.image.env`（固定表头注释 + 7 行赋值），保证幂等、可 diff。
文件头注明「机器生成，手工改动会在下次发布时被覆盖」。

## 7. 校验与门禁

| 检查 | 手段 |
| --- | --- |
| `.image.env` 与发布版本一致 / 文件自洽 | `node scripts/update-image-env.js -v <ver> --check`（不给 `-v` 时从现文件反推版本做自洽校验）——CI `deploy_image_env_check` job |
| compose 的 `${IMAGE_*}` 变量集合 == `.image.env` 声明的变量集合 | `scripts/update-image-env.test.js` 单测（compose 与 env 漂移会红） |
| 镜像真的在 registry 里 | `node scripts/update-image-env.js -v <ver> --verify` |

> 注：仓库已有的 `scripts/check-compose-images.js` 在 2026-11-09 的 scripts 瘦身中
> 被移除（它绑定的是旧的 `infrastructure/` 布局）；等价能力由上面的单测 +
> `docker compose --env-file .image.env -f docker-compose-services.yml config` 覆盖。

## 8. 风险

| 风险 | 缓解 |
| --- | --- |
| 忘了更新 `.image.env` → 部署的还是旧镜像 | `--check` 门禁 + `.image.env` 入库（diff 里看得见版本变化） |
| `.image.env` 与 compose 变量名拼错 → compose 硬报错 | `:?` 语法 + `check-compose-images.js` |
| 脚本误改 compose 文件 | **不改 compose**，只写 `.image.env`；compose 侧只有变量名，永不变 |
| `.env` 里的旧 `IMAGE_TAG_*` 残留造成双真相源 | 本次从 `.env` / `.env.example` 删除，并加注释指向 `.image.env` |

## 9. 实施清单

| # | 文件 | 动作 |
| --- | --- | --- |
| 1 | `examples/docker-deploy/.image.env` | 新增（入库） |
| 2 | `examples/docker-deploy/docker-compose-services.yml` | `image:` → `${IMAGE_*:?...}` |
| 3 | `examples/docker-deploy/.env` / `.env.example` | 删 `kudi_repos` / `IMAGE_TAG_PG` / `IMAGE_TAG_GATEWAY` |
| 4 | `examples/docker-deploy/start.sh` | 加载 `.image.env`、校验镜像变量、透传 `--env-file` |
| 5 | `scripts/update-image-env.js` | 新增（含 `--check` / `--verify`） |
| 6 | `scripts/update-image-env.test.js` | 新增（node:test：compose↔env 变量集合一致 + 文件幂等） |
| 7 | `.gitlab-ci.yml` | 新增 `deploy_image_env_check` job（test 阶段，阻断式） |
| 8 | `package.json` | 加 `deploy:image-env` / `deploy:image-env:check` + 纳入 `test:scripts` |
| 9 | `docs/deploy-topologies.md` / `AGENTS.md` / `.gitignore` | 同步文档；忽略 `examples/*/.env` |

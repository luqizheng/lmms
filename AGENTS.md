# coder-micro-services AI 协作指南

> 深度上下文（服务地图、架构、完整 pitfall 列表、ADR 索引）见 [`docs-site/ai-context.md`](docs-site/ai-context.md) — 接手本仓库时**先读它**。
> 强制规则（分支/提交、spec 纪律、路径触发器、CI 门禁）见 [`.trae/rules/project_rules.md`](.trae/rules/project_rules.md)。本文件只收最高频、最易踩坑的要点。

## 语言偏好

AI 的**思考过程、文档撰写、对话回复必须使用中文**。代码注释、commit message、技术术语保持英文。同一规则写入 `.trae/rule/project_rules.md` §1.1，全仓库（含 `.trae/rules/*`）统一执行。

## 开发环境

- **日常开发：编译 + 测试在主机上跑**（不进 Docker 编译容器，增量编译复用宿主 target 目录）：
  ```bash
  node scripts/run-tests-host.js                 # 全部服务
  node scripts/run-tests-host.js iam-service     # 单个/多个服务
  node scripts/run-tests-host.js --incremental   # 只跑 git 改动涉及的服务
  node scripts/run-tests-host.js --no-db         # 纯单测，不连数据库
  node scripts/build-cargo.js --debug            # 主机侧编译（--release 为默认）
  ```
  Windows 上默认在 **WSL2 发行版**里执行 cargo（`scripts/lib/host-runner.js` 负责选发行版、
  转路径、注入 env）；Linux/macOS 直接本机 cargo（`--backend native` 可强制）。
  WSL 与 Windows/Docker 的 TCP 在镜像网络下互通，测试直接连 Docker 里的共享 MySQL
  （`127.0.0.1:33306`）与 Redis（`127.0.0.1:36379`），无需在 WSL 里另装数据库。
  设计细节见 `docs/superpowers/specs/2026-10-27-host-native-build-test-design.md`。
- **CI 与发布前回归：集成测试（含 MySQL / testcontainers）跑容器**：
  ```bash
  node scripts/run-tests-docker.js                 # 全部服务
  node scripts/run-tests-docker.js iam-service     # 单个/多个服务
  ```
  基础镜像 `zhcoder-docker-registry.com:8000/builder/rust:1.96-slim-musl`，target `x86_64-unknown-linux-musl`，源码 bind mount、`registry`/`git`/`target` 命名 volume 复用增量编译。TLS 统一 `reqwest 0.12 + rustls`，禁用 native-tls。
  发布镜像（多架构 / musl）只能走 `node packages/cli/bin/coder.js build images`，主机侧产物与之不等价。
- **纯单元测试可以脱离容器跑**（前提：本机有 `cc`/`gcc` + `rustup target add x86_64-unknown-linux-gnu`，与 docker 共用的 `target/` 是 root-owned 时用独立 dir 绕开）：
  ```bash
  # 一次性：把 swagger-ui zip 预下载到 infrastructure/vendor/，避免 build.rs 触发网络
  pnpm run assets:download

  # 单 service 的纯单测（src/ 下的 #[test] / #[tokio::test]），不依赖 MySQL
  CARGO_TARGET_DIR=/tmp/<svc>-target \
    SWAGGER_UI_DOWNLOAD_URL=file://$(pwd)/infrastructure/vendor/swagger-ui-v5.17.14.zip \
    cargo test -p <svc> --lib

  # 集成测试（tests/ 下、用 testcontainers 起 MySQL）仍需 docker daemon 起容器；
  # run-tests-docker.js 已自动挂载 vendor zip，无需手设 ENV
  ```
  两个常见坑：
  - `utoipa-swagger-ui` build.rs 默认直连 GitHub 拉 zip，本机 SSL 拦截 → 先跑 `pnpm run assets:download`，再设 `SWAGGER_UI_DOWNLOAD_URL=file://...`；`scripts/run-tests-docker.js` 与 `scripts/run-tests-host.js` 都会自动注入。
  - 不要 `sudo chown` repo 根目录的 `target/`——会被 docker volume `coder-cargo-target` 抢回去。用 `CARGO_TARGET_DIR` 指向用户-writable 目录。
- **Windows 原生 MSVC 编译不可用**：`ring` / `aws-lc-sys` 的 C 依赖在 cargo build script 里调 `cl.exe` 会报 `D8050 无法加载 c1.dll`（不在 VS 开发环境里则 `C1083 stddef.h`）。Windows 上请走 WSL2 后端。
- crates.io 已在 `.cargo/config` 替换为 `rsproxy.cn` 镜像；CI 走内网 `crates.kudi.repos`。
- 起本地开发：`pnpm run dev:infra`（Consul/Redis/MySQL）→ `pnpm run dev`。`pnpm run check:env` 检查环境。

## Git 与提交规范

- **任务分支优先**：新阶段功能 / 验证工作从 `main` 新建描述性分支（如 `validation/software-project-management`、`feat/<scope>`）；**一个任务一个 commit**；**禁止 amend / force-push**。
- Commit message：`<Phase>:<summary>` 或 `feat(scope): <summary>`（如 `docs(plan): ...`、`fix: ...`）。
- **改代码前先有 spec**：`docs/superpowers/specs/YYYY-MM-DD-<topic>-design.md`，同提交周期内创建。

## CI 门禁（push 前必须全过）

```bash
node scripts/check-service-standard.js      # 服务规范（services/**）
node scripts/check-spec.js                  # OpenAPI / 错误码 / 权限码（services/**/openapi.yaml）
node scripts/contracts-gen.js --check       # 契约漂移（contracts/**）
node scripts/check-db-naming.js             # 表名服务前缀（migrations + sqlx 查询）
node scripts/dump-openapi-specs.js && git diff --exit-code -- 'services/*/openapi.yaml'
```

GitLab CI `test` 阶段跑其中部分门禁（service-standard / spec / contracts / openapi-drift）+ `cargo test --workspace` + E2E；失败阻断 build/deploy。也可装 pre-commit（`.pre-commit-config.yaml`）本地拦截。

## 契约层与代码生成

`contracts/` 是前后端单一真相源：`error-codes.yaml` / `permissions.yaml` / `events.yaml` / `http-conventions.md` / `db-conventions.md`。

- 改 YAML 后跑 `node scripts/contracts-gen.js` 重新生成 `libs/digua_common-error/src/codes.rs`、`libs/digua_authz/src/perms.rs`。**禁止手改这两个生成文件**。
- **表名必须带服务前缀**（共享 MySQL `superpowers` 库）：`iam_` / `audit_` / `token_` / `ntf_` / `cfg_` / `file_` / `prue_`。规则见 `contracts/db-conventions.md`，违例由 `check-db-naming.js` 拦截。

## 前端集成

**框架不随附前端控制台**（PRISM console 已于 2026-10 从本仓库移除）。前端项目位于各自业务仓，通过以下方式接入 api-gateway（唯一公共入口，host port 3000）：

- **TypeScript / 浏览器**：仓库顶层 `packages/@digua/contracts`（错误码 / 权限码 / OpenAPI 客户端 TS 产物）、`packages/@digua/http`（框架无关 HTTP 客户端，token 注入 / 单飞刷新 / 错误码转文案）与 `sdk/iam-services`（登录 / 用户管理 / token 管理 SDK；**位置例外**：npm TS 包放 `sdk/` 经用户裁决 2026-10-31）。发布到私有 registry **`http://192.168.2.12:14873/`**（`npm.kudi.repos` 为同一 Verdaccio 的域名别名；canonical 以守卫常量 `packages/cli/lib/npm-packages.js::REGISTRY` 为准）。发布用 `pnpm run packages:publish`，校验用 `pnpm run packages:check`，规则见「npm 包发布规则」。CI `packages_test` job 守门。
- **服务端 SDK**：`sdk/java`（Maven）、`sdk/rust`（kellnr）；C# SDK（`sdk/dotnet`）规划中，尚未创建。

## OpenAPI（auto-generated）

`services/*/openapi.yaml` 由 utoipa 从代码生成，**禁止直接编辑**。改路由后：编辑 `#[utoipa::path]` 注解与 `src/openapi.rs` → 跑 `node scripts/dump-openapi-specs.js` 重新生成 → commit。CI 用 `git diff --exit-code` 检测漂移。

> utoipa 若要调 `to_yaml()`，服务 `Cargo.toml` 必须带 `features = [..., "yaml"]`，否则编译失败。

## 数据库迁移

- **默认 forward-only**：不编辑已发布迁移，新增 `V{N+1}__*.sql`（CI 与 `project_rules.md` 强制）。
- prue_backend 必须同时出 MySQL + SQLite 两版（`migrations/{mysql,sqlite}/`）；其余服务仅 MySQL。
- 项目处于开发阶段、**无生产数据**：可大胆重构、删废弃代码，无需向后兼容；大规模重构时可整体重建迁移集（无数据需保留）。

## 身份认证

遵循 [`.trae/rules/auth-rules.md`](.trae/rules/auth-rules.md)：

- **禁止使用 `X-User-*` headers**；下游服务从 `Authorization: Bearer <jwt>` 解码 payload（不验签，信任网关已验签）。
- 用 `libs/jwt-ctx::Principal` extractor 取身份。网关负责 RSA 验签 + Redis 黑名单 + exp 检查，Authorization 原样透传。

## 服务规范

每个 `services/<svc>/` 下有 `AGENTS.md` 声明本服务约定（端口、DB、端点域、env、豁免项）— **改该服务前先读**。8-item 规范覆盖 Dockerfile / Swagger / 健康检查 / 迁移 / README / tests 等。关键 env：`SWAGGER_ENABLED`、`HEALTH_CHECK_REQUIRED`、`MIGRATIONS_REQUIRED`。

> 跨服务审计写不再走直连 HTTP，走 RabbitMQ + transactional outbox（`digua_common_event::MySqlOutboxRepo::enqueue`，同事务入队）。详见 `docs-site/ai-context.md` §7 pitfall 19。

## Skill 目录

Skill 分**下游(downstream)** 与 **仓库 repo-local / 平台**两类；路径触发器见 `project_rules.md` §4。白名单权威在 `release.js::DOWNSTREAM_SKILL_WHITELIST`（仓库 `.agents/skills/` 始终全量保留，发布副本只带下列下游项）。

### ① 下游（downstream）——发布分发包 `.agents/skills/` 只带这 6 个，供下游创建/维护 Java & Rust 业务微服务

| 意图 | Skill（仓库 `.agents/skills/…`） | 备注 / 常用命令 |
| --- | --- | --- |
| 新建 Rust 服务 | `coder-new-service` | 8-item 服务标准 |
| 加路由 | `coder-add-route` | 改完跑 `node scripts/dump-openapi-specs.js` |
| 加迁移 | `coder-migrate-db` | Rust 双（MySQL/SQLite）语义迁移 |
| 新建 Java 服务 | `coder-new-java-service` | Spring Boot 3 + JDK 21 |
| 加路由 | `coder-add-java-route` | 加 Controller/DTO |
| 加迁移 | `coder-migrate-java-db` | Java Flyway 迁移 |

### ② 仓库 repo-local / 平台（本仓开发用，**不随 dist 分发**；给下游新增需先在 `DOWNSTREAM_SKILL_WHITELIST` 追加）

| 意图 | Skill | 常用命令 |
| --- | --- | --- |
| 发布 npm TS 包 | `pnpm run packages:publish`（CI `publish_packages` 随 tag 自动） | `pnpm run packages:build` |
| 发布 Rust SDK | `.agents/skills/coder-sdk-publish-rust` | `pnpm run sdk:publish:rust -- <VERSION>` |
| 发布 Java SDK | `.agents/skills/coder-sdk-publish-maven` | `pnpm run sdk:publish:release` |
| 打离线分发包 / 服务镜像 | `.agents/skills/coder-service-release` | `pnpm run release:build` / `node release/dist-template/scripts/release.js` |
| CI 红标诊断 | `.agents/skills/coder-ci-diagnose` | — |
| 跨服务事件链路 | `.agents/skills/coder-contract-trace` | — |
| 迁移回滚 | `.agents/skills/coder-migration-rollback` | 紧急 forward-only 回滚 |
| OpenAPI/契约校验 | `.agents/skills/coder-openapi-contracts-sync` | 改路由后跑 5 门禁 |
| 纯/Rust 单测 | `.agents/skills/coder-rust-unit-test` | — |
| 性能剖析 | `.agents/skills/coder-perf-profile` | — |
| 审查 PR | `.agents/skills/coder-microservice-review-checklist` | — |
| 通用编码守则 | `.agents/skills/karpathy-guidelines` | — |
| **PRISM 设计约束** | `.agents/skills/prism-design` | 写任何前端 UI 前必读（前端在业务仓，框架不随附） |
| **新业务系统前端** | `.agents/skills/scaffold` | 拷贝壳 + 装基础包 + 品牌化（业务仓前端，经 `packages/*` 接入网关） |
| **壳升级同步** | `.agents/skills/update-shell` | 上游壳 diff 同步到下游 |
| **生成业务页面** | `.agents/skills/gen-page` | 列表页 + 弹框编辑页 |
| 审查计划 | `.agents/skills/plan-eng-review` | — |
| 同步基础镜像 | `.trae/skills/coder-base-images-sync`（平台） | `node scripts/base-images-sync.js` |
| 审计服务规范 | `.trae/skills/coder-service-audit`（平台） | `node scripts/check-service-standard.js` |
| 跑 E2E | `.trae/skills/coder-e2e-test`（平台） | `node scripts/run-e2e-local.js` |
| 升级通用库 | `.trae/skills/coder-lib-upgrade`（平台） | `cargo upgrade -p <lib>` |
| 引导开发环境 | `.trae/skills/coder-dev-bootstrap`（平台） | — |

## 网络规则

- 从 GitHub 拉文件（releases / archives / raw）必须走代理：`https://gh-proxy.org/<原始链接>`。
- 内网镜像仓库 `zhcoder-docker-registry.com:8000`。

## 跨仓库分发（包更新模型）

本仓库是**多个下游消费仓的上游**。代表下游：`project-manager` 等业务垂直集成仓。

**分发模型（2026-10-30 起）：发版 = 镜像 + 各种包，更新全部走包升级。** 下游不再做源码级同步（原 `cms-sync` / `@digua/dist-tools` 已停用并从发布链路移除）：

- **镜像**：`zhcoder-docker-registry.com:8000/micro-server/<svc>:<ver>`，下游改 compose 的 image tag 后 `docker compose pull && up -d`。
- **Rust**：`libs/*`（17 crate）与 `sdk/rust/*`（7 crate）发布到 kellnr（`http://192.168.2.12:16444`），下游 `cargo update -p <crate>`。
- **Java**：`sdk/java` 发布到 Maven（`http://mvn.kudi.repos`），下游升 `<dependency>` 版本。
- **npm TS**：`packages/@digua/contracts` + `@digua/http` + `sdk/iam-services` 发布到 `http://192.168.2.12:14873/`，下游 `pnpm up @digua/contracts @digua/http` / `pnpm add iam-services`。

tag 流水线五路并行发布（`sdk_publish` / `rust_publish` / `build_images:tagged` / `publish_packages`，`release:dist` 收尾打离线包）——见 `.gitlab-ci.yml` publish stage。同 tag 下所有制品版本号一致。

**npm 包发布规则**（守卫实现在 `packages/cli/lib/npm-packages.js`，本文与其保持一致）：

1. **新增包必须三处接线**，缺一则 tag CI 与本地发布都会**静默跳过**该包：① `pnpm-workspace.yaml` 清单；② 根 `package.json` 的 `packages:build` 过滤；③ `npm-packages.js` 的 `PACKAGES` 数组。
2. 发布守卫四条：`publishConfig.registry` 必须**精确等于** `REGISTRY`（含尾斜杠）；`dist/index.js` 已构建；包目录 `git status` 干净；包内版本 < 目标版本。
3. **版本单一事实源 = git tag（或 `--version` 参数）**；仓内 `package.json` 版本只是占位符，本地发布后需 `git checkout -- <各包>/package.json` 还原占位。
4. 发布一律走 CLI（先 `pnpm run packages:check` 干跑，再 `pnpm run packages:publish --version x.y.z`），**勿手敲 `npm publish --prefix`** —— npm≥11 会从 cwd 读 manifest 直接崩（CLI 已改为在包目录内执行，并显式钉死 registry 防 `@scope:registry` 劫持）。发布机 `~/.npmrc` 需有 `//192.168.2.12:14873/:_authToken`。

### 修改本仓 `infrastructure/**` 时对下游的影响

下游不再 merge 本仓 compose 源文件（只在**新建环境**时用 `release/dist-template` 起步），但 airgap 包内的 compose 仍从本仓生成：

- `release/dist-template/scripts/transform-compose.js` 只把 `build:` 块换成 `image:` 引用，**不动 `command:` 字段**。
- 所以**当下游报告 OCI `executable file not found in $PATH`，而镜像里实际装了该 binary 时，根因几乎 100% 在本仓 compose**。

### Cargo `[[bin]] name` → 容器内二进制名的强制映射

`[[bin]] name = "x-y-z"`（kebab-case）在 cargo build 时**强制**把磁盘产物名转为下划线 `x_y_z`（[Cargo Book · Targets > Binaries](https://doc.rust-lang.org/cargo/reference/cargo-targets.html#binaries)）。本仓实例：

```toml
# services/notification-service/Cargo.toml
[[bin]] name = "notification-service"    # ↦ /usr/local/bin/notification_service
[[bin]] name = "ntf-outbox-publisher"     # ↦ /usr/local/bin/ntf_outbox_publisher
```

docker-compose `command:` 字段**必须**用下划线。常见踩坑：把 `ntf-outbox-publisher`(kebab)直接抄进 compose（`["ntf-outbox-publisher"]`），启动时 OCI runtime 在 `$PATH` 中找不到对应名，容器创建失败。

### 发布命令与镜像 tag 约定（v1.5.0 校准）

- 离线分发包统一入口：`node release/dist-template/scripts/release.js <VERSION>`（原 `release.sh` 已于 2026-08 迁 Node 删除）；只打配置/骨架包加 `--no-images`。单测：`pnpm run release:test`。
- **git tag 带 `v`，发布镜像 tag 不带 v**：CI `build_images:tagged` 用 `${CI_COMMIT_TAG#v}` 剥 v 后推送，registry 里实际存在的是 `micro-server/<svc>:1.5.0`（**不是** `:v1.5.0`）。下游 dist-template `.env` 的 `IMAGE_TAG` 必须填 no-v 值，填 `v1.5.0` 会 pull 不到镜像。
- airgap bundle 导出清单见 `release.js` 内 `IMAGES` 数组：除 9 个平台服务外，**v1.5.0 起还含 `mysql-replica`**（tag pipeline 由 CI buildx 单独推送，主 compose 引用它）与 **alertmanager**（公网代理完整地址，内网 registry 未同步）。离线环境报缺镜像 → 先查该清单是否漏导。

### 发布前对账 SOP

```bash
# 1) 渲染离线下发包 compose（不变 command:）
node release/dist-template/scripts/transform-compose.js \
  ./infrastructure/docker-compose-services.yml \
  zhcoder-docker-registry.com:8000 1.5.0

# 2) 列出真实二进制名（与 compose 的 command: 字段交叉对账）
docker run --rm --entrypoint sh \
  zhcoder-docker-registry.com:8000/micro-server/<svc>:v1.0.0 \
  -c 'ls /usr/local/bin'
```

任何镜像 `command:` 与 `ls /usr/local/bin` 输出出现拼写差异 → 修本仓 compose 后重 release，不允许"先推了再说"。

## SDK 发布

所有 SDK 发布脚本已改为 Node.js 实现，跨平台兼容（Linux / macOS / Windows）。

| 发布内容 | 命令 | Registry |
|---------|------|----------|
| npm 包 (`@digua/contracts` + `@digua/http`) | `pnpm run packages:publish`（CI `publish_packages` 随 tag 自动） | `npm.kudi.repos` |
| Rust 通用库 (17 crates) | `pnpm run sdk:publish:libs -- <VERSION>` | `kellnr` |
| Rust SDK (7 crates) | `pnpm run sdk:publish:rust -- <VERSION>` | `kellnr` |
| Java SDK (Maven) | `pnpm run sdk:publish:release` / `:snapshot` | 私有 Reposilite |

> step-by-step 发布 playbook（含 dry-run / 认证细节）见 `.agents/skills/coder-sdk-publish-{npm,rust,maven}`；离线分发包见 `coder-service-release`。

> `target/` 目录是 docker volume root-owned，发布 Rust crate 时必须设置 `CARGO_TARGET_DIR`：
> ```bash
> # Linux / macOS
> CARGO_TARGET_DIR=/tmp/libs-target pnpm run sdk:publish:libs -- 1.0.0
> 
> # Windows PowerShell
> $env:CARGO_TARGET_DIR="$env:TEMP\libs-target"; pnpm run sdk:publish:rust -- 1.0.0
> ```
>
> 发布前先 dry-run 验证：加 `--dry-run` 参数。

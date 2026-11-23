# coder-micro-services AI 协作指南

> 深度上下文（服务地图、架构、完整 pitfall 列表、ADR 索引）见 [`docs-site/ai-context.md`](docs-site/ai-context.md) — 接手本仓库时**先读它**。
> 强制规则（分支/提交、spec 纪律、路径触发器、CI 门禁）见 [`.trae/rules/project_rules.md`](.trae/rules/project_rules.md)。本文件只收最高频、最易踩坑的要点。

## 语言偏好

AI 的**思考过程、文档撰写、对话回复必须使用中文**。代码注释、commit message、技术术语保持英文。同一规则写入 `.trae/rule/project_rules.md` §1.1，全仓库（含 `.trae/rules/*`）统一执行。

## 回复风格

你是高效助手。规则：

1. 只回答必要信息，不寒暄、不重复问题。
2. 默认用要点/表格/代码，不写长段落。
3. 不确定先问1个关键问题；信息够就直接做。
4. 输出不超过我要求的字数/条数；未指定则尽量≤200字。
5. 不复述上下文，不解释显而易见的内容。

## 开发环境

- **日常开发：编译 + 测试在主机上跑**（不进 Docker 编译容器，增量编译复用宿主 target 目录）：
  ```bash
  node scripts/build-cargo.js --debug                   # 全部 service，debug
  node scripts/build-cargo.js --debug token-service    # 单个 service
  node scripts/build-cargo.js                          # 全部 service，release
  ```
  **2026-11-09 起统一走 host 本机 cargo，不再走 WSL2 发行版**：
  - Windows：host 原生 MSVC 工具链。`scripts/lib/host-runner.js` 自动定位
    Visual Studio 的 `vcvars64.bat` 并注入 `INCLUDE` / `LIB` / `PATH`——否则
    `ring` / `aws-lc-sys` 的 C 依赖会在 build script 阶段报 `D8050 无法加载 c1.dll`
    / `C1083 stddef.h`。装了 VS 2022 / VS 18 任一版本的 Build Tools 即可，脚本自动发现。
  - Linux / macOS：host 原生 gcc/clang，直接本机 cargo。
  - 测试直接连 Docker 里的共享 MySQL（`127.0.0.1:33306`）与 Redis（`127.0.0.1:36379`）。
  - 设计细节见 `docs/superpowers/specs/2026-10-27-host-native-build-test-design.md`。
- **CI 与发布前回归：集成测试（含 MySQL / testcontainers）跑容器**：
  ```bash
  node scripts/run-tests-docker.js                 # 全部服务
  node scripts/run-tests-docker.js iam-service     # 单个/多个服务
  ```
  基础镜像 `zhcoder-docker-registry.com:8000/builder/rust:1.96-slim-musl`，target `x86_64-unknown-linux-musl`，源码 bind mount、`registry`/`git`/`target` 命名 volume 复用增量编译。TLS 统一 `reqwest 0.12 + rustls`，禁用 native-tls。
  发布镜像（多架构 / musl）只能走 `node packages/cli/bin/coder.js build images`，主机侧产物与之不等价。
- **纯单元测试可以脱离容器跑**（前提：本机有 MSVC Build Tools / cc / gcc）：
  ```bash
  # 一次性：把 swagger-ui zip 预下载到 infrastructure/vendor/，避免 build.rs 触发网络
  pnpm run assets:download

  # 单 service 的纯单测（src/ 下的 #[test] / #[tokio::test]），不依赖 MySQL
  # Windows（PowerShell）：
  $env:SWAGGER_UI_DOWNLOAD_URL="file:///d:/kudi-project/templates/low-memory-micro-services/infrastructure/vendor/swagger-ui-v5.17.14.zip"
  cargo test -p token_service --lib
  ```
  两个常见坑：
  - `utoipa-swagger-ui` build.rs 默认直连 GitHub 拉 zip，本机 SSL 拦截 → 先跑 `pnpm run assets:download`，再设 `SWAGGER_UI_DOWNLOAD_URL=file://...`；`scripts/build-cargo.js` 会自动注入。
  - 不要 `sudo chown` repo 根目录的 `target/`——会被 docker volume `coder-cargo-target` 抢回去。用 `CARGO_TARGET_DIR` 指向用户-writable 目录。
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

- **TypeScript / 浏览器**：仓库顶层 `packages/@digua/contracts`（错误码 / 权限码 / OpenAPI 客户端 TS 产物）、`packages/@digua/http`（框架无关 HTTP 客户端，token 注入 / 单飞刷新 / 错误码转文案），以及 `sdk/<svc>-services` 五个微服务 SDK（`config-services` 已随 config-service 服务移除，2026-11-06）：`iam-services`（登录/用户管理/token 管理）、`audit-services`（日志/合规导出/保留策略）、`file-services`（multipart 上传/下载/预签名）、`notification-services`（公告/未读数，不含 WS）、`prue-services`（动态路由/菜单配置）——npm TS 包放 `sdk/` 系**位置例外**（用户裁决 2026-10-31）。CI `packages_test` job 守门。
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

Skill 分**下游(downstream)** 与 **仓库 repo-local / 平台**两类；路径触发器见 `project_rules.md` §4。

### ① 下游（downstream）——供下游创建/维护 Java & Rust 业务微服务

| 意图 | Skill（仓库 `.agents/skills/…`） | 备注 / 常用命令 |
| --- | --- | --- |
| 新建 Rust 服务 | `coder-new-service` | 8-item 服务标准 |
| 加路由 | `coder-add-route` | 改完跑 `node scripts/dump-openapi-specs.js` |
| 加迁移 | `coder-migrate-db` | Rust 双（MySQL/SQLite）语义迁移 |
| 新建 Java 服务 | `coder-new-java-service` | Spring Boot 3 + JDK 21 |
| 加路由 | `coder-add-java-route` | 加 Controller/DTO |
| 加迁移 | `coder-migrate-java-db` | Java Flyway 迁移 |

### ② 仓库 repo-local / 平台（本仓开发用）

| 意图 | Skill | 常用命令 |
| --- | --- | --- |
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

**分发模型：发版 = 镜像 + 各种包，更新全部走包升级。** 下游不再做源码级同步：

- **镜像**：`zhcoder-docker-registry.com:8000/micro-server/<svc>:<ver>`，下游改 compose 的 image tag 后 `docker compose pull && up -d`。
- **Rust**：公开发行通过 GitHub tag workflow 发布到 crates.io；内部消费场景仍可按环境配置私有 registry。下游 `cargo update -p <crate>`。
- **Java**：`sdk/java` 发布到 Maven（`http://mvn.kudi.repos`），下游升 `<dependency>` 版本。
- **npm TS**：公开发行通过 GitHub tag workflow 发布到 npmjs.org；包清单的 `publishConfig` 指向官方 registry。下游 `pnpm up @digua/contracts @digua/http` / `pnpm add <svc>-services`。

- **git tag 带 `v`，发布镜像 tag 不带 v**：CI `build_images:tagged` 用 `${CI_COMMIT_TAG#v}` 剥 v 后推送，registry 里实际存在的是 `micro-server/<svc>:1.5.0`（**不是** `:v1.5.0`）。下游 `.env` 的 `IMAGE_TAG` 必须填 no-v 值。
- **部署单元的镜像钉版在 `.image.env`**（2026-11-09）：两个示例单元（`examples/docker-deploy/` services-only、`examples/docker-deploy-full/` 全栈含 infra+observability）的 compose 都只写整名变量 `${IMAGE_*}`，值在各自 `.image.env`（入库，无凭据）。**版本号真相源是根 `Cargo.toml` 的 `[workspace.package].version`**（8 个 `services/*` 用 `version.workspace = true` 继承）；`release.toml` 只放方言/镜像源/SDK 目标。平台镜像段机器生成（两单元逐字一致），基础镜像段（full 独有）手工维护、重生成时保留。发布：`coder release images --push`（读服务版本 → 构建 → 成功后写两个单元的 `.image.env`）；体检：`coder release status`；SDK：`coder release sdk rust|npm|java|all`。详见 `docs/deploy-topologies.md`「镜像引用与版本钉版」。
- `mysql-replica` 已于 2026-11-05 移除（生产数据拓扑归 DB 组，lmms 只带单节点开发/测试库）。

### Cargo `[[bin]] name` → 容器内二进制名的强制映射

`[[bin]] name = "x-y-z"`（kebab-case）在 cargo build 时**强制**把磁盘产物名转为下划线 `x_y_z`（[Cargo Book · Targets > Binaries](https://doc.rust-lang.org/cargo/reference/cargo-targets.html#binaries)）。本仓实例：

```toml
# services/notification-service/Cargo.toml
[[bin]] name = "notification-service"    # ↦ /usr/local/bin/notification_service
[[bin]] name = "ntf-outbox-publisher"     # ↦ /usr/local/bin/ntf_outbox_publisher
```

docker-compose `command:` 字段**必须**用下划线。常见踩坑：把 `ntf-outbox-publisher`(kebab)直接抄进 compose（`["ntf-outbox-publisher"]`），启动时 OCI runtime 在 `$PATH` 中找不到对应名，容器创建失败。

# 开发者工作流总图

> **一份图看完整个开发者动作链**：拉代码 → 改 → 测 → 推 → CI → 出镜像 → 部署 → 观测。
>
> 每一步都标出 *做什么 / 跑哪个命令 / 详细知识跳到哪里*。本文不重复细节，只做**索引**。
>
> **最近更新**：2026-07-09 ｜ **适用范围**：coder-micro-services 全仓库
>
> 💡 **新人入职**：如果你是第一次接触这个仓库，先走 [`docs-site/onboarding/README.md`](onboarding/README.md) 的 9 章节线性入门，再回到这份总图进入日常工作节奏。

---

## 0. 一张图看完

```
                                  开发者工作流（端到端）

 ┌────────────────┐   ┌────────────────┐   ┌────────────────┐   ┌────────────────┐
 │ Phase 0        │   │ Phase 1        │   │ Phase 2        │   │ Phase 3        │
 │ 拿到代码       │──▶│ 写代码前       │──▶│ 实现           │──▶│ 本地测试       │
 │ (Day 1, 一次)  │   │ (每次任务)     │   │ (每次任务)     │   │ (每次任务)     │
 └────────────────┘   └────────────────┘   └────────────────┘   └────────────────┘
                                                                          │
                                                                          ▼
 ┌────────────────┐   ┌────────────────┐   ┌────────────────┐   ┌────────────────┐
 │ Phase 9        │   │ Phase 8        │   │ Phase 7        │   │ Phase 4-6      │
 │ 上线后观测     │◀──│ 部署 / 灰度    │◀──│ 出镜像         │◀──│ 提交 / 推送    │
 │ (持续)         │   │ (按发布窗口)   │   │ (CI 自动 +     │   │ (每次任务)     │
 │                │   │                │   │  本地 hotfix)  │   │                │
 └────────────────┘   └────────────────┘   └────────────────┘   └────────────────┘

粗体黑箭头 = 每次任务必经 细箭头 = 按需 / 一次性 / 持续
```

> 💡 老员工把这份图当作速查表，按当前所在阶段往下翻。
> 新员工建议配合 [`docs-site/onboarding/README.md`](onboarding/README.md) 通读一遍。

---

## 1. Phase 0 — 拿到代码（Day 1，仅一次）

**一句话**：装好工具 → 拉代码 → **预拉基镜像** → 启基础设施 → 跑通一个 hello 接口。

```bash
git clone <repo-url> coder-micro-services
cd coder-micro-services
pnpm run setup                                            # install + 基镜像清单 sync + 预拉基镜像 + check:env
                                                          # 等价于手工四步：pnpm install && pnpm run images:sync && pnpm run images:pull && pnpm run check:env
docker compose -f infrastructure/docker-compose.yml up -d   # 启 MySQL/Redis/Consul
cd services/iam-service && cargo run                        # 跑第一个服务（首次 5–15 min）
```

> 💡 为什么不是 `pnpm install`？Day 1 真正缺的是**基镜像**（无网/认证/镜像冷拉都痛）。`pnpm run setup` 自动按 [`infrastructure/base-images.json`](../infrastructure/base-images.json) 走 mirror → source 回落到本地 Docker 缓存。

**详细文档**：
- 工具清单、安装步骤、IDE 配置 → [`docs-site/onboarding/02-环境准备.md`](onboarding/02-环境准备.md)
- 从 clone 到接口正常返回（每步在做什么） → [`docs-site/onboarding/03-第一次跑通.md`](onboarding/03-第一次跑通.md)
- 项目结构、技术栈、服务端口表 → [`docs-site/onboarding/01-项目概览.md`](onboarding/01-项目概览.md)

---

## 2. Phase 1 — 写代码前（每次任务）

**一句话**：先写 spec（设计文档），再读目标 service 的 AGENTS.md，确认方案。

```bash
# 1) 在 docs/superpowers/specs/ 新建规格
vim docs/superpowers/specs/$(date +%Y-%m-%d)-<topic>-design.md

# 2) 改哪个服务就读哪个的 AGENTS.md
cat services/<svc>/AGENTS.md
```

**为什么先写 spec**：[`.trae/rules/project_rules.md`](../.trae/rules/project_rules.md) §1.3 把 spec 列为强制规则。
**AI 协作者必读**：[`docs-site/ai-context.md`](ai-context.md)（服务地图、坑位清单、skill 目录）。

---

## 3. Phase 2 — 实现（每次任务）

**一句话**：能用 skill 的就不手写；改完立即跑 `cargo check` 看诊断。

| 想做的事 | 用哪个 skill / 工具 |
|---|---|
| 给某服务加接口 | `.agents/skills/coder-add-route/`（自动生成 utoipa 注解 + openapi.rs 注册 + 测试 stub） |
| 加表 / 加列 / 改 schema | `.agents/skills/coder-migrate-db/`（MySQL + SQLite 双版本） |
| 新建一个微服务 | `.agents/skills/coder-new-service/`（满足 8-item 服务规范） |
| 审一遍方案 | `.agents/skills/plan-eng-review/` |

```bash
# 静态检查（每次改完就跑）
cargo check -p <svc>
# 配合 IDE：VS Code rust-analyzer / JetBrains RustRover
```

**每个 Rust 服务都有自己的约定**（端口、env、例外项） → `services/<svc>/AGENTS.md`，改之前必读。

---

## 4. Phase 3 — 本地测试（每次任务）

**一句话**：先静态，再单元，再集成，最后 smoke e2e —— 由快到慢逐层加。

| 层级 | 命令 | 耗时 |
|---|---|---|
| **静态** | `cargo check` / `lsp_diagnostics` | ~5s |
| **单元** | `cd services/<svc> && cargo test --lib` | ~5s |
| **集成** | `cd services/<svc> && cargo test --test integration_test` | ~30s（testcontainers 起容器） |
| **E2E smoke** | `pnpm -C tests/e2e test:smoke`（8 API + 2 UI） | ~2 min |

```bash
# 智能调用（推荐；本机不需要 cc linker，bind mount 进 docker）
node scripts/run-tests-docker.js <svc>         # 单个服务
node scripts/run-tests-docker.js                # 全部服务
```

**详细文档**：
- 测试金字塔、数据驱动 YAML、smoke 选样、报告 → [`docs-site/guides/testing.md`](guides/testing.md)
- 改 SQL 后必跑 → `cargo sqlx prepare` 更新 `sqlx-data.json`（[`docs-site/onboarding/05-数据库与迁移.md`](onboarding/05-数据库与迁移.md) §5.5）

---

## 5. Phase 4 — 提交前 4 道 CI Gate（push 前必须全过）

**一句话**：4 条命令，全过才允许 push；任何一条挂则本地修复，绝不上 CI 跑死循环。

```bash
node scripts/check-service-standard.js          # ① 8-item 服务规范（Dockerfile/AGENTS.md/...）
node scripts/check-spec.js                      # ② OpenAPI / 权限码 / 错误码
node scripts/contracts-gen.js --check           # ③ 契约生成代码与 YAML 一致
node scripts/dump-openapi-specs.js                   # ④ 重新生成 openapi.yaml
git diff --exit-code -- 'services/*/openapi.yaml'   # ④ 确认无 drift
```

**强制规则出处**：[`.trae/rules/project_rules.md`](../.trae/rules/project_rules.md) §1.5（CI Gates）。

> ⚠️ 跳过这一步直接 push，结果就是 CI 红 + 自己 review 自己的循环，得不偿失。

---

## 6. Phase 5 — 提交 & 推送

**一句话**：新阶段功能 / 验证工作先从 `main` 新建描述性任务分支（如 `validation/...`、`feat/...`），完成后合并回 `main`；一个任务一个 commit；不打 amend 不打 force-push。

```bash
git add <精确的文件，别 git add .>
git commit -m "feat(<scope>): <一句话 imperative 描述>"
git push origin main
```

| 限制 | 出处 |
|---|---|
| **任务分支优先**（从 `main` 新建 `feat/<scope>` / `validation/<topic>` 等） | `.trae/rules/project_rules.md` §1.2 |
| **一个 task 一个 commit** | 同上 |
| **禁止 amend / force-push** | 同上 |
| **commit 格式**：`<Phase>:<summary>` 或 `feat(scope): ...` | 同上 |
| **不要提交秘钥**（`.env`、`*.pem`、`keys/`） | 已 gitignore，但心里有数 |

---

## 7. Phase 6 — CI 自动跑（push 后立即看到）

**一句话**：CI 会自动跑 `test → build → deploy/publish`，等红灯 / 绿灯即可。

| Stage | Job | 干啥 |
|---|---|---|
| `test` | `rust_test`, `service_standard_check`, `spec_check`, `contracts_check`, `spec_generate`, `e2e_api_test`, `e2e_ui_test` | 静态检查 + 单元/集成 + e2e |
| `build` | `build_api_gateway`, `build_token_service`, ... | Docker build + push（`:`sha` + `:latest` 两个 tag） |
| `deploy` | `deploy`（仅 master 分支） | 在 `192.168.2.128` 上 `docker compose pull & up -d` |
| `publish` | `sdk_publish`（仅打 tag `vX.Y.Z` 触发） | Java SDK 推到内网 Maven |

配置文件：[`.gitlab-ci.yml`](../.gitlab-ci.yml)。

---

## 8. Phase 7 — 出镜像（Image Publish）

**两套路径**：日常走 CI 自动；hotfix / 紧急补丁走本地手动。

**前置**：基镜像清单由 [`infrastructure/base-images.json`](../infrastructure/base-images.json) 单一真相源管理（按 Dockerfile / compose yml 扫描生成）。想换基镜像版本 / 加新基镜像 → skill `.trae/skills/coder-base-images-sync/`（`pnpm run images:sync`）。预拉本地缓存 → `pnpm run images:pull`（或在 Phase 0 一次性跑 `pnpm run setup`）。

### 8.1 路径 A：CI 自动（推荐，日常用）

> 正常流程：**Phase 5 push → CI `build_*` job 自动 `docker buildx build` + `docker push`**。
>
> 镜像自动推到 `zhcoder-docker-registry.com:8000/micro-server/<svc>:<sha>` + `:latest`。

不用任何额外命令，看 CI 即可。

### 8.2 路径 B：本地手动出镜像（hotfix / RC / 调试时用）

`scripts/build-docker.js` 支持交互式 / 参数式两种模式，自带版本号 tag、多 CPU 架构、可选推送。

```bash
# 交互式（无参数）：依次问 版本号 / 架构 / 是否推送 / 服务子集
pnpm docker:build

# 参数式：全部服务，版本号 1.0.0，直接 buildx --push（不留本地）
pnpm docker:build -- -v 1.0.0 --push

# 本地留镜像 + 推送（单架构 --load 后再 docker push 每 tag）
pnpm docker:build -- --load-and-push -v 1.0.0

# 多架构（buildx 多架构无法 --load，自动开启 --push）
pnpm docker:build -- --platform linux/amd64,linux/arm64 --push

# 单服务子集
pnpm docker:build -- api-gateway token-service -v 1.0.0 --push
# = node scripts/build-docker.js api-gateway token-service -v 1.0.0 --push
```

| 工具 | 干啥 |
|---|---|
| [`scripts/build-docker.js`](../scripts/build-docker.js) | 批量构建 + 推送 micro-server 镜像。无参数=交互式；参数式支持 `-v <ver>`（同时打 `:<ver>` + `:latest`）、`--push`（buildx 直推）、`--load-and-push`（单架构本地留镜像 + 推送）、`-p/--platform linux/amd64,linux/arm64`（多架构，自动 `--push`）。默认 registry `zhcoder-docker-registry.com:8000/micro-server`。`node scripts/build-docker.js --help` 看全部选项。 |
| [`scripts/build-docker.js clean`](../scripts/build-docker.js) | 清理 buildx cache（`cargo-registry`、`cargo-git`）。 |

**模式与默认值**：

| 模式 | 触发 | push 默认 | 架构默认 |
|---|---|---|---|
| 交互式 | 无参数 / `-i` | Y（推送） | linux/amd64 |
| 参数式 | 带任何参数 | 不推送（需 `--push`/`--load-and-push`） | linux/amd64 |

**多架构注意**：buildx 多架构镜像无法 `--load` 到本地 docker，必须 `--push`。`--platform` 含多个架构且未显式推送时，脚本自动开启 `--push`；`--load-and-push` 在多架构下回退为 `--push`。arm64 构建需宿主启用 qemu binfmt：`docker run --privileged --rm tonistiigi/binfmt --install all`。

**非 TTY**：CI 等 stdin 非 TTY 环境无参数时回退到参数式默认值（latest / amd64 / 不推送 / 全部服务），不阻塞。

**registry**：`zhcoder-docker-registry.com:8000/micro-server/<service>`
**基镜像**：[`infrastructure/base-images.json`](../infrastructure/base-images.json) 列出 7 个 builder 镜像（`node:20` / `rust:1.96-slim-musl` / ...）

### 8.3 出问题怎么排查镜像问题

| 现象 | 排查 |
|---|---|
| `docker buildx` 报错拉不到基础镜像 | 检查 `~/.docker/config.json` 中 `zhcoder-docker-registry.com:8000` 凭证 |
| 镜像 tag 推不上去（401 / connection refused） | 同上 + `docker login zhcoder-docker-registry.com:8000` |
| 镜像能 push 跑不起来 | 多半 base image 选错（musl vs gnu），看 [`infrastructure/base-images.json`](../infrastructure/base-images.json) |
| 想换基镜像版本 | 用 skill `.trae/skills/coder-base-images-sync/`（`node scripts/base-images-sync.js`） |

---

## 9. Phase 8 — 部署与灰度发布

**一句话**：master 自动部署；想做灰度走 Consul tag。

### 9.1 自动部署

CI 的 `deploy` job（`.gitlab-ci.yml`）只在 `master` 分支触发，在 `192.168.2.128` 跑 `docker compose pull && up -d`。**日常 push main 即部署**。

手动部署或切换版本：

```bash
ssh kudi@192.168.2.128
cd /opt/coder-micro-services
git pull origin main                   # 拉最新代码
docker-compose down && docker-compose pull && docker-compose up -d
```

详细：[`../README.md` CI/CD Deployment 章节](../README.md#ci-cd-deployment)。

### 9.2 灰度发布

想验证新版本但不立刻切全量 → 灰度（10% → 50% → 100%）。

**详细分步操作、回滚、调整比例** → [`docs-site/guides/canary-rollout.md`](guides/canary-rollout.md)

### 9.3 改基础设施 / ConfigMap

- 改 `infrastructure/config/api-gateway/routes/*.router.toml` → `curl -X POST http://localhost:3000/__admin/reload`（热加载，详见 [`docs-site/guides/api-gateway-routing.md`](guides/api-gateway-routing.md)）
- 改 `rate-limits.toml` → 同上热加载

---

## 10. Phase 9 — 上线后观测（持续）

**一句话**：出问题时 4 步定位 — 看 Jaeger trace → grep 日志 → 查 Grafana → 查 Consul 服务注册。

| 你想知道 | 工具 | 地址 |
|---|---|---|
| 一次请求经过哪些服务、谁慢谁挂 | **Jaeger**（trace_id） | http://localhost:16686 |
| QPS / 错误率 / 限流 / P99 | **Grafana**（admin/admin） | http://localhost:3001 |
| 实时日志 | `docker logs -f <container>` 或 `cargo run` 终端 | — |
| 服务健康状态 | **Consul UI** | http://localhost:8500 |

**详细文档**：
- 跨服务 trace 怎么用 → [`docs-site/guides/otel-tracing.md`](guides/otel-tracing.md)
- 指标看板 + PromQL → [`docs-site/guides/observability.md`](guides/observability.md)
- 5 个最常见错误的应对 → [`docs-site/onboarding/07-日志与排查Bug.md`](onboarding/07-日志与排查Bug.md)
- 请求头里拿 `X-Trace-Id` → 同上 §7.4

---

## 11. 卡住时

按这个顺序自检：[`docs-site/onboarding/09-出问题怎么办.md`](onboarding/09-出问题怎么办.md) §9.1 的清单。
还是搞不定 → 用 [`docs-site/onboarding/09-出问题怎么办.md`](onboarding/09-出问题怎么办.md) §9.3 的 bug 报告模板提 issue。

---

## 12. 速查表（一行命令）

| 任务 | 命令 |
|---|---|
| 启基础设施 | `docker compose -f infrastructure/docker-compose.yml up -d` |
| Day 1 一键 setup (install + 基镜像 pull + check:env) | `pnpm run setup` |
| 预拉基镜像（手动，与 setup 等价分步） | `pnpm run images:sync && pnpm run images:pull` |
| 启可观测性 (Prometheus/Grafana/Jaeger/OTel) | `docker compose -f infrastructure/docker-compose-observability.yml up -d` |
| 跑某个服务 | `cd services/<svc> && cargo run` |
| 跑某个服务测试 | `node scripts/run-tests-docker.js <svc>` |
| 跑 smoke e2e | `pnpm -C tests/e2e test:smoke` |
| 重新生成 OpenAPI | `node scripts/dump-openapi-specs.js` |
| 跑 4 道 CI Gate | `node scripts/check-service-standard.js && node scripts/check-spec.js && node scripts/contracts-gen.js --check && node scripts/dump-openapi-specs.js && git diff --exit-code -- 'services/*/openapi.yaml'` |
| 单独出某个镜像 | `pnpm docker:build:<svc>` |
| 全部出镜像 | `pnpm docker:build` |
| 出镜像并打版本号推送 | `pnpm docker:build -- -v 1.0.0 --push` |
| 重新加载 gateway 路由 | `curl -X POST http://localhost:3000/__admin/reload` |
| 发布 Rust libs (17 crates) | `CARGO_TARGET_DIR=/tmp/libs-target node scripts/publish-libs.js <VER>` |
| 发布 Rust SDK (7 crates) | `CARGO_TARGET_DIR=/tmp/sdk-target node scripts/publish-rust-sdks.js <VER>` |
| 发布 npm 包 (`@digua/contracts` + `@digua/http`) | `pnpm run packages:build && pnpm run packages:publish` |

---

## 13. 关键路径索引

| 我想… | 去哪 |
|---|---|
| 我是新人，第一次来 | [`docs-site/onboarding/README.md`](onboarding/README.md) |
| 我是 AI Agent，第一次进仓库 | [`docs-site/ai-context.md`](ai-context.md) |
| 我要改代码但不熟规则 | [`.trae/rules/project_rules.md`](../.trae/rules/project_rules.md) |
| 我要给某服务加接口 | `.agents/skills/coder-add-route/` |
| 我要加数据库表 / 列 | `.agents/skills/coder-migrate-db/` |
| 我要新建一个微服务 | `.agents/skills/coder-new-service/` |
| 我想知道测试怎么搞 / 怎么写 e2e | [`docs-site/guides/testing.md`](guides/testing.md) |
| 我要发布 SDK | [`docs-site/guides/rust-sdk-publishing.md`](guides/rust-sdk-publishing.md) |
| 我要灰度发布 | [`docs-site/guides/canary-rollout.md`](guides/canary-rollout.md) |
| 我要查 api-gateway 路由配置 | [`docs-site/guides/api-gateway-routing.md`](guides/api-gateway-routing.md) |
| 我要看怎么排查 bug | [`docs-site/onboarding/07-日志与排查Bug.md`](onboarding/07-日志与排查Bug.md) |
| 我要看协议层的错误码 / 权限码 | [`../contracts/`](../contracts/) |
| 我要想清楚再写的 spec 模板 | [`docs/superpowers/specs/`](superpowers/specs/) |

---

> **反馈**: 本文档与代码库同步维护。发现某条命令过期、链接失效或阶段缺失 → 提 issue 或直接 PR。

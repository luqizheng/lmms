# coder-micro-services AI 协作指南

> 深度上下文（服务地图、架构、完整 pitfall 列表、ADR 索引）见 [`docs/ai-context.md`](docs/ai-context.md) — 接手本仓库时**先读它**。
> 强制规则（分支/提交、spec 纪律、路径触发器、CI 门禁）见 [`.trae/rules/project_rules.md`](.trae/rules/project_rules.md)。本文件只收最高频、最易踩坑的要点。

## 语言偏好

AI 的**思考过程、文档撰写、对话回复必须使用中文**。代码注释、commit message、技术术语保持英文。同一规则写入 `.trae/rules/project_rules.md` §1.1，全仓库（含 `.trae/rules/*`）统一执行。

## 开发环境

- 类 Unix（WSL / macOS / Linux）。Windows 必须 WSL2，禁止原生 PowerShell / CMD。
- **本地 `cargo` 通常编译不过**（缺 `cc` 链接器、musl target）。跑 Rust 测试统一用容器：
  ```bash
  node scripts/run-tests-docker.js                 # 全部服务
  node scripts/run-tests-docker.js iam-service     # 单个/多个服务
  ```
  基础镜像 `zhcoder-docker-registry.com:8000/builder/rust:1.96-slim-musl`，target `x86_64-unknown-linux-musl`，源码 bind mount、`registry`/`git`/`target` 命名 volume 复用增量编译。TLS 统一 `reqwest 0.12 + rustls`，禁用 native-tls。
- crates.io 已在 `.cargo/config` 替换为 `rsproxy.cn` 镜像；CI 走内网 `crates.kudi.repos`。
- 起本地开发：`pnpm run dev:infra`（Consul/Redis/MySQL）→ `pnpm run dev`。`pnpm run check:env` 检查环境。

## Git 与提交规范

- **直接在 `main` 上工作**，不开 feature 分支；**一个任务一个 commit**；**禁止 amend / force-push**。
- Commit message：`<Phase>:<summary>` 或 `feat(scope): <summary>`（如 `docs(plan): ...`、`fix: ...`）。
- **改代码前先有 spec**：`docs/superpowers/specs/YYYY-MM-DD-<topic>-design.md`，同提交周期内创建。

## CI 门禁（push 前必须全过）

```bash
node scripts/check-service-standard.js      # 服务规范（services/**）
node scripts/check-spec.js                  # OpenAPI / 错误码 / 权限码（services/**/openapi.yaml）
node scripts/contracts-gen.js --check       # 契约漂移（contracts/**）
node scripts/check-db-naming.js             # 表名服务前缀（migrations + sqlx 查询）
scripts/dump-openapi-specs.sh && git diff --exit-code -- 'services/*/openapi.yaml'
```

GitLab CI `test` 阶段跑其中部分门禁（service-standard / spec / contracts / openapi-drift）+ `cargo test --workspace` + E2E；失败阻断 build/deploy。也可装 pre-commit（`.pre-commit-config.yaml`）本地拦截。

## 契约层与代码生成

`contracts/` 是前后端单一真相源：`error-codes.yaml` / `permissions.yaml` / `events.yaml` / `http-conventions.md` / `db-conventions.md`。

- 改 YAML 后跑 `node scripts/contracts-gen.js` 重新生成 `libs/common-error/src/codes.rs`、`libs/authz/src/perms.rs`。**禁止手改这两个生成文件**。
- **表名必须带服务前缀**（共享 MySQL `superpowers` 库）：`iam_` / `audit_` / `token_` / `ntf_` / `cfg_` / `file_` / `prue_`。规则见 `contracts/db-conventions.md`，违例由 `check-db-naming.js` 拦截。

## OpenAPI（auto-generated）

`services/*/openapi.yaml` 由 utoipa 从代码生成，**禁止直接编辑**。改路由后：编辑 `#[utoipa::path]` 注解与 `src/openapi.rs` → 跑 `scripts/dump-openapi-specs.sh` 重新生成 → commit。CI 用 `git diff --exit-code` 检测漂移。

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

> 跨服务审计写不再走直连 HTTP，走 RabbitMQ + transactional outbox（`common_event::MySqlOutboxRepo::enqueue`，同事务入队）。详见 `docs/ai-context.md` §7 pitfall 19。

## Skill 目录

项目 skill 在 `.agents/skills/`，平台 skill 在 `.trae/skills/`（路径触发器见 `project_rules.md` §4）：

| 意图 | Skill | 常用命令 |
| --- | --- | --- |
| 加路由 | `.agents/skills/coder-add-route` | 改完跑 `scripts/dump-openapi-specs.sh` |
| 加迁移 | `.agents/skills/coder-migrate-db` | — |
| 新建服务 | `.agents/skills/coder-new-service` | — |
| 审查计划 | `.agents/skills/plan-eng-review` | — |
| 同步基础镜像 | `.trae/skills/coder-base-images-sync` | `node scripts/base-images-sync.js` |
| 审计服务规范 | `.trae/skills/coder-service-audit` | `node scripts/check-service-standard.js` |
| 跑 E2E | `.trae/skills/coder-e2e-test` | `node scripts/run-e2e-local.js` |
| 升级通用库 | `.trae/skills/coder-lib-upgrade` | `cargo upgrade -p <lib>` |
| 引导开发环境 | `.trae/skills/coder-dev-bootstrap` | — |

## 网络规则

- 从 GitHub 拉文件（releases / archives / raw）必须走代理：`https://gh-proxy.org/<原始链接>`。
- 内网镜像仓库 `zhcoder-docker-registry.com:8000`。

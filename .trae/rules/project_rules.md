# coder-micro-services 项目规则

> AI 协作触发规则。Trae 会在改动匹配路径时，自动建议对应的 skill。

---

## 路径 → Skill 自动匹配

| 改动路径                                         | 建议 Skill                        | 时机      |
| ------------------------------------------------ | --------------------------------- | --------- |
| `services/*/src/**.rs`                           | `coder-run-tests` -Service <name> | 改完保存  |
| `services/*/tests/**.rs`                         | `coder-run-tests` -Service <name> | 改完保存  |
| `docker/config/api-gateway/routes/*.router.toml` | `coder-route-validate`            | 改完保存  |
| `tests/specs/**.yaml`                            | `coder-spec-to-code`              | 新建/修改 |
| `sdk/java/**/pom.xml`                            | `coder-sdk-publish` (test)        | 改完保存  |
| `sdk/dotnet/**/*.csproj`                         | `coder-sdk-publish` (test)        | 改完保存  |
| `infrastructure/docker-compose.*.yaml`           | `coder-microservice-dev`          | 改完保存  |
| `services/*/AGENTS.md`                            | 自动检查服务规范                  | 新建/修改 |

服务名清单：`token-service` / `api-gateway` / `notification-service` / `audit-service` / `config-service` / `file-service` / `iam-service`

---

## 失败 → Skill 自动匹配

| 触发条件                                         | 建议 Skill                        |
| ------------------------------------------------ | --------------------------------- |
| `reports/run-*/summary.md` 出现 fail > 0         | `coder-fix-tests`                 |
| `reports/latest-fix-prompt.md` 存在且 mtime < 1h | `coder-fix-tests`                 |
| `cargo build` 失败                               | 先 `cargo check` 看错，不直接 fix |

---

## 强约束（AI 必须遵守）

### 修改边界

- AI 触发 `coder-fix-tests` 时**仅改**：
  - Rust: `services/<service>/tests/*.rs`
- **禁止改**：`services/`、`libs/`、`sdk/` 下非 tests 目录的业务源码

### 服务规范检查

- AI 修改 `services/<name>/` 下文件时，必须先读取 `AGENTS.md`
- 缺失必需组件时提醒用户
- 环境变量控制规范要求（SWAGGER_ENABLED、HEALTH_CHECK_REQUIRED、MIGRATIONS_REQUIRED）

### 路由配置

- 改 `routes/*.router.toml` 后**必须**先 `validate` 再 commit
- 新增路由必须确认 `match` 唯一（启动会报 `duplicate match pattern`）
- `rewrite` 类型只能是 `strip_prefix` / `redirect` / `header_set` / `header_add` / `path_param`

### Docker 基础设施

- 网络抖动时用 `docker compose up -d --pull never` 绕过镜像拉取
- 本地镜像 `redis:7-alpine` / `mysql:8.0` / `consul:1.15` 已缓存
- 不要修改 `scripts/lib/Docker-Helper.js` 的 `prePull`，那是网络正常时的优化

### Rust 工具链

- 仓库已切到 `aws_lc_rs` crypto provider
- 不要回退到 `ring`，会触发 MSVC LNK2019 (fiat_25519 链接错误)
- **沙箱限制**：Trae IDE 沙箱不加载 vcvarsall，`cargo build` / `cargo test` 会 LNK2019。**所有 cargo 命令必须在真实 PowerShell（vcvarsall x64）跑**，不要在 IDE 终端内直接 `cargo`
- 沙箱内允许：`Get-Content`、`Select-String`、`git status`/`log`/`diff` 等只读命令

### Node.js 编排脚本

- 所有基础脚本统一为 Node.js：入口 `scripts/test-orchestrator.js`，lib 在 `scripts/lib/*.js`
- 仅依赖 Node.js 内置模块（`node:fs` / `node:path` / `node:child_process`），不引入第三方包
- 旧 `.ps1` 已删除；新加脚本请用 `node scripts/xxx.js` 入口形式

---

## 提交规范

- 改 `routes/` → commit message 前缀 `feat(api-gateway):` 或 `fix(api-gateway):`
- 改 `services/<svc>/` → 前缀 `feat(<svc>):` / `fix(<svc>):`
- 改 `sdk/` → 前缀 `feat(sdk):` / `fix(sdk):`
- 改 `scripts/` → 前缀 `chore(scripts):` 或 `feat(scripts):`
- 改 `.trae/` → 前缀 `chore(ai):`

---

## 命令检查清单

每次 commit 前自动跑（如果 AI 改了对应路径）：

```bash
# Rust 改动
cargo fmt --check
cargo clippy --all-targets -- -D warnings

# 路由改动
cargo run -p api-gateway -- validate --routes docker\config\api-gateway\routes

# YAML 规格改动
node scripts/test-orchestrator.js validate-specs
```

---

## 不做的事

- ❌ 不要主动 `git push`，除非用户明说
- ❌ 不要主动 `git commit`，除非用户明说
- ❌ 不要修 `services/<svc>/src/` 下的业务源码，除非用户授权
- ❌ 不要把 `private.pem` 加进 commit
- ❌ 不要在跑 test 时手动改 `reports/` 下的文件
- ❌ 不要给 SDK 包发版（CI tag 触发，不在本地 push）

---

## 速查

- 跑测试：`node scripts/test-orchestrator.js loop`
- 看上次报告：`Get-Content reports\run-*\summary.md | Select-Object -Last 1`
- 校验路由：`cargo run -p api-gateway -- validate --routes docker\config\api-gateway\routes`
- 起本地服务：见 `coder-microservice-dev` skill

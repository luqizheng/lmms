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

服务名清单：`token-service` / `api-gateway` / `credential-login-service` / `user-service` / `notification-service`

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

### 路由配置

- 改 `routes/*.router.toml` 后**必须**先 `validate` 再 commit
- 新增路由必须确认 `match` 唯一（启动会报 `duplicate match pattern`）
- `rewrite` 类型只能是 `strip_prefix` / `redirect` / `header_set` / `header_add` / `path_param`

### Docker 基础设施

- 网络抖动时用 `docker compose up -d --pull never` 绕过镜像拉取
- 本地镜像 `redis:7-alpine` / `mysql:8.0` / `consul:1.15` 已缓存
- 不要修改 `Docker-Helper.ps1` 的 `Invoke-PrePull`，那是网络正常时的优化

### Rust 工具链

- 仓库已切到 `aws_lc_rs` crypto provider
- 不要回退到 `ring`，会触发 MSVC LNK2019 (fiat_25519 链接错误)

### PowerShell

- 所有脚本入口在 `scripts/test-orchestrator.ps1`
- `lib/*.ps1` 用 dot-source，不要用 `Import-Module`
- 中文输出用 UTF-8 BOM；当前 `summary.md` 已有编码问题待修

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

```powershell
# Rust 改动
cargo fmt --check
cargo clippy --all-targets -- -D warnings

# 路由改动
cargo run -p api-gateway -- validate --routes docker\config\api-gateway\routes

# YAML 规格改动
.\scripts\test-orchestrator.ps1 validate-specs
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

- 跑测试：`.\scripts\test-orchestrator.ps1 loop`
- 看上次报告：`Get-Content reports\run-*\summary.md | Select-Object -Last 1`
- 校验路由：`cargo run -p api-gateway -- validate --routes docker\config\api-gateway\routes`
- 起本地服务：见 `coder-microservice-dev` skill

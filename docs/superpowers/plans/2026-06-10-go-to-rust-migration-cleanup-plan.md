# Go→Rust 迁移残留清理 — 实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 清理仓库内 Go→Rust 迁移未覆盖的字符串、路径、配置、脚本残留；不改业务逻辑。

**Architecture:** 不新增 crate / service；纯字符串替换、文件改名、孤儿配置删除。

**背景：** 大部分迁移工作由 [`2026-06-10-credential-login-user-rust-plan.md`](file:///d:/projects/coder-micro-services/docs/superpowers/plans/2026-06-10-credential-login-user-rust-plan.md) 完成；本计划只收拾遗漏项。已被取代的旧 plan 标为 Superseded。

**改名映射**（执行任何清理前必读）：

| 旧名 | 新名 | 当前状态 |
|---|---|---|
| `auth-service` | `credential-login-service` | **stub**（`src/lib.rs` 仅 `// placeholder`；T5 Step 2 删 compose 块的依据）|
| `token-server` | `token-service` | 已完成；本 plan 处理所有 `token-server` 字面量与路径 |

**stub 清单**（不要尝试补全业务代码）：
- `services/credential-login-service` — 仅 `Cargo.toml` + 空 `src/lib.rs`；业务由独立 plan 负责

**Task 3 例外授权**：`services/token-service/src/{main,consul}.rs` 的修改仅替换字面量字符串（config 路径、日志、`service_id` 前缀），**不触碰任何业务逻辑**。已获用户明文授权（cleanup plan 评审通过）。

**Stop on first failure**：单 Task 任一 Step 验证失败（cargo check / cargo test / git grep / YAML 解析 / PowerShell 语法）→ **停止整个 plan 推进**，回到失败 Step 排查。不允许「先 commit 后续 task 再回头修」。

**沙箱限制**：`cargo check` / `cargo test` / `cargo fmt` / `cargo clippy` 必须在真实 PowerShell（vcvarsall x64）跑。Trae IDE 沙箱内 `cargo` 会触发 LNK2019。仅 `Get-Content` / `Select-String` / `git status|log|diff` / `python -c` 等只读命令允许在沙箱执行。

**命令规范**：
- `cargo test` → 走 `coder-run-tests` skill（`-Service <name>`）
- 路由校验 → `coder-route-validate` skill（本 plan 不涉及路由修改，但 T6 残留校验用 `git grep` 兜底）
- YAML 校验 → `.\scripts\test-orchestrator.ps1 validate-specs`（本 plan 不涉及 spec 文件，跳过）

**Tech Stack:** 不涉及新依赖；纯文本编辑 + PowerShell 验证。

---

## 文件结构

| 路径 | 改动 |
|---|---|
| `scripts/run_tests.ps1` | 删 `Test-GoService`；改 `token-server` 路径 |
| `package.json` | 删 Go build/push 脚本；改 `token-server` → `token-service` |
| `services/token-service/src/main.rs` | 改 config 路径与日志字符串 |
| `services/token-service/src/consul.rs` | 改硬编码 `"token-server"` |
| `docker/config/token-server/` | 改名为 `token-service/` |
| `docker/config/auth-service/config.yaml` | 删（孤儿）|
| `docker/config/token-server.toml` | 改名为 `token-service.toml` |
| `docker/docker-compose.yml` | `token-server` 块改 `token-service`；删 `auth-service` 块（credential-login-service 仍是 stub）|

---

## Task 1: scripts/run_tests.ps1 清理

**Files:**
- Modify: `scripts/run_tests.ps1`

- [ ] **Step 1: 删 `Test-GoService` 函数（lines 34-49）**

整段函数（含三处 `go test` 调用）删除。

- [ ] **Step 2: 替换所有 `Test-GoService` 调用**

`$ProjectRoot\services\auth-service` / `services\user-service` 处的 `Test-GoService` 调用（lines 61-62, 69-70, 79-80）整行删除。

- [ ] **Step 3: 改 `token-server` 路径**

把 line 60 / 68 / 78 的 `services\token-server` 改为 `services\token-service`。

- [ ] **Step 4: 校验脚本无 Go 残留**

```powershell
Select-String -Path scripts\run_tests.ps1 -Pattern "go test|Test-GoService|services\\token-server|services\\auth-service|services\\user-service"
```

Expected: 无输出

- [ ] **Step 5: 跑 dry-run**

```powershell
& {
  function Test-RustService { param($a,$b) Write-Host "Test-RustService $b" }
  . {
    $ProjectRoot = "d:\projects\coder-micro-services"
    & "$ProjectRoot\scripts\run_tests.ps1" -Unit -ErrorAction Stop
  }
}
```

Expected: 列出 `API Gateway (Rust)` 与 `Token Service (Rust)`，无 Go 报错。

- [ ] **Step 6: 提交**

```powershell
git add scripts\run_tests.ps1
git commit -m "chore(scripts): drop go test branches and fix token-service path"
```

---

## Task 2: package.json 清理

**Files:**
- Modify: `package.json`

- [ ] **Step 1: 改 `build`（line 6）**

```json
"build": "cargo build --release --manifest-path services/api-gateway/Cargo.toml && cargo build --release --manifest-path services/token-service/Cargo.toml",
```

（删 `token-server` → `token-service`；删两处 `cd services/auth-service && go build` 与 `user-service`）

- [ ] **Step 2: 替换 `docker:build:token-server`（line 19）**

```json
"docker:build:token-service": "docker build -f Dockerfile.token-service -t zhcoder-docker-registry.com:8000/micro-server/token-service .",
```

- [ ] **Step 3: 删 `docker:build:auth-service`（line 20）**

整行删除。

**`docker:build:user-service` 保留**——`services/user-service/` 是现有 Rust 服务（`services/user-service/Dockerfile` 是 rust 1.81 builder），不是 Go。**plan 原稿写「删 user-service」是错的**。

- [ ] **Step 4: 改聚合 `docker:build`（line 22）**

```json
"docker:build": "pnpm run docker:build:api-gateway && pnpm run docker:build:token-service",
```

（删 `auth-service` / `user-service` 串；改 `token-server` → `token-service`）

- [ ] **Step 5: 改 `docker:push:*`（lines 24-26）+ 聚合（line 27）**

对称调整：删 `auth-service` / `user-service` 整行；`token-server` → `token-service`；聚合脚本保留 2 项。

- [ ] **Step 6: 校验 JSON**

```powershell
Get-Content package.json | ConvertFrom-Json | Out-Null
```

Expected: 无异常。

- [ ] **Step 7: 提交**

```powershell
git add package.json
git commit -m "chore(scripts): drop go docker/pnpm scripts; align token-service"
```

---

## Task 3: services/token-service 源码字符串清理

**Files:**
- Modify: `services/token-service/src/main.rs`
- Modify: `services/token-service/src/consul.rs`

- [ ] **Step 1: 改 main.rs line 28 的 config 路径**

`config/token-server` → `config/token-service`。

- [ ] **Step 2: 改 main.rs line 57 的日志标题**

`"========== token-server config =========="` → `"========== token-service config =========="`。

- [ ] **Step 3: 改 main.rs line 148 / 161 的 tracing 字面量**

`"token-server registered with consul"` → `"token-service registered with consul"`。
`"token-server listening on {}"` → `"token-service listening on {}"`。

- [ ] **Step 4: 改 consul.rs line 57 的硬编码 service id 前缀**

`format!("{}-{}", "token-server", Uuid::new_v4())` → `format!("{}-{}", "token-service", Uuid::new_v4())`。

- [ ] **Step 5: 验证编译（真实 PowerShell）**

```powershell
# 真实 PowerShell（vcvarsall x64），非沙箱
cargo check -p token-service
```

Expected: 通过。

- [ ] **Step 6: 跑现有测试（走 skill）**

```powershell
# 真实 PowerShell，调用 coder-run-tests skill
.\scripts\test-orchestrator.ps1 test-all -Service token-service
```

Expected: 全部通过（unit + integration 都跑）。

- [ ] **Step 7: 提交**

```powershell
git add services\token-service
git commit -m "refactor(token-service): rename residual token-server strings"
```

---

## Task 4: docker config 改名

**Files:**
- Rename: `docker/config/token-server/` → `docker/config/token-service/`
- Rename: `docker/config/token-server.toml` → `docker/config/token-service.toml`
- Delete: `docker/config/auth-service/config.yaml`

- [ ] **Step 1: 改名目录**

```powershell
git mv docker\config\token-server docker\config\token-service
```

- [ ] **Step 2: 改名顶层 toml**

```powershell
git mv docker\config\token-server.toml docker\config\token-service.toml
```

- [ ] **Step 3: 删孤儿 auth-service 配置**

```powershell
git rm docker\config\auth-service\config.yaml
rmdir docker\config\auth-service
```

- [ ] **Step 4: 全仓校验旧名**

```powershell
git grep -n "config/token-server" -- ':!Cargo.lock' ':!docs/superpowers/plans/2026-06-10-go-to-rust-migration-plan.md' ':!docs/superpowers/specs/2026-06-10-go-to-rust-migration-design.md'
```

Expected: 仅命中 Task 3 Step 1 已改的 `main.rs`（应为空，因已改）；若还命中 `docker/` 内文件，下个 Task 处理。

- [ ] **Step 5: 提交**

```powershell
git add docker\config
git commit -m "chore(docker): rename token-server config dir; drop orphan auth-service"
```

---

## Task 5: docker/docker-compose.yml 清理

**Files:**
- Modify: `docker/docker-compose.yml`

- [ ] **Step 1: 改 `token-server` 服务块（lines 69-103）**

逐项替换：
- `token-server:`（顶层 key）→ `token-service:`
- `image: zhcoder-docker-registry.com:8000/micro-server/token-server` → `image: zhcoder-docker-registry.com:8000/micro-server/token-service`
- `- "4000:4000"` 保留（端口沿用）
- `volumes:` 中 `./config/token-server/:/app/config/` → `./config/token-service/:/app/config/`
- `environment:`：
  - `RUST_LOG=token_server=debug` 保留
  - `TS_CONSUL__SERVICE_NAME=token-server` → `TS_CONSUL__SERVICE_NAME=token-service`
  - `TS_CONSUL__SERVICE_ID_FILE=/tmp/token-server-id` → `TS_CONSUL__SERVICE_ID_FILE=/tmp/token-service-id`
  - 其余 `TS_` 变量保留（已用 `__` 分隔符，是 Rust 风格）
- `depends_on:` 中 `token-server:` → `token-service:`
- `healthcheck.test` 中 `localhost:4000` 保留

- [ ] **Step 2: 删 `auth-service` 整块（lines 138-175）**

`auth-service:` 到 `networks: - superpowers` 整段删除。`auth-service` 当前是 stub（`services/credential-login-service/src/lib.rs` 仍是 `// placeholder`），compose 内挂上无意义；待 `credential-login-service-rust-plan` 实施完再补回。

- [ ] **Step 3: 改 `user-service` 块（lines 177-208）**

`services/user-service/src/main.rs:14` 通过 `load_config("config/config.yaml")` 读 yaml，**不读环境变量**。所有 `SERVER_*` / `DATABASE_*` / `CONSUL_*` env vars 是 Go 时代残留，全是死代码。处理：

- `environment:` 整段删除
- 保留 `volumes: ../services/user-service/config:/app/config`（这才是真实配置源）
- 保留 `ports: "6000:6000"`（**已知 bug**：yaml 是 8082，Dockerfile 也是 EXPOSE 8082，compose 端口错。**不**在本 plan 修，记入 Step 5 「已知问题」）
- 保留 `image: .../micro-server/user-service`
- 保留 `depends_on: mysql / consul`
- `healthcheck.test: ...localhost:6000/health` 同步改为 `localhost:8082/health`（避免误报 unhealthy）

> 原 plan T5 Step 3 写的是把 `SERVER_HOST` → `US_SERVER__HOST`（加 `US_` 前缀改 env 风格）。**整段作废**——Rust 服务根本不读 env。

- [ ] **Step 4: 校验 YAML**

```powershell
python -c "import yaml; yaml.safe_load(open('docker/docker-compose.yml'))"
```

Expected: 无异常。

- [ ] **Step 5: 提交**

```powershell
git add docker\docker-compose.yml
git commit -m "chore(docker): rename token-server block; drop auth-service; align user-service env"
```

---

## Task 6: 全仓 Go 残留校验

- [ ] **Step 1: 扫已跟踪文件**

```powershell
git ls-files | Select-String -Pattern "\.go$|go\.mod|go\.sum|go\.work"
```

Expected: 无输出。

- [ ] **Step 2: 扫 Go 关键字（仅业务源码与脚本，文档/历史 plan 忽略）**

```powershell
git grep -lE "\bgo test\b|\bgo build\b" -- 'scripts/*.ps1' 'package.json' 'services/' 'libs/' 'infrastructure/' ':!docs/superpowers/plans' ':!docs/superpowers/specs'
```

Expected: 无输出。

- [ ] **Step 3: 扫旧服务名**

```powershell
git grep -lE "token-server|services/auth-service|services\\auth-service|/auth-service/Dockerfile" -- 'scripts/' 'package.json' 'services/' 'infrastructure/' 'docker/' '.gitlab-ci.yml' 'Dockerfile.*' ':!docs/superpowers/plans' ':!docs/superpowers/specs'
```

Expected: 无输出。

- [ ] **Step 4: 若有命中，单独处理**

每个命中文件按上下文改，不在本 plan 范围内扩散。

---

## Task 7: 收尾

- [ ] **Step 1: fmt / clippy（真实 PowerShell）**

```powershell
# 真实 PowerShell（vcvarsall x64），非沙箱
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
```

Expected: 通过。

- [ ] **Step 2: 全 workspace 测试（走 skill）**

```powershell
# 真实 PowerShell，调用 coder-run-tests skill（等价于对 Get-ServiceList 5 个服务全跑）
.\scripts\test-orchestrator.ps1 test-all
```

Expected: 全部通过（credential-login-service 仍是 stub，编译占位不影响其他服务；integration 测试需 Docker 已起，否则 fail 是预期的，跳过看 unit）。

- [ ] **Step 3: 提交（如有 fmt/clippy 改动）**

```powershell
git add -A
git commit -m "style(workspace): rustfmt and clippy fixes"
```

---

## 自审

- **范围控制**：仅字符串替换 / 路径改名 / 孤儿删除；不碰业务逻辑；不改任何 service 的 `src/` 内函数（除 `token-service` 的字面量与 config 路径）。
- **可执行性**：每步 1-3 分钟；全 plan 改完后跑 Task 6 三道校验即可确认无残留。
- **依赖**：Task 1-3 互不依赖，可并行；Task 4-5 依赖 Task 1-3 完成后跑（避免又出现新字面量）；Task 6-7 必须在最后。
- **回滚**：每 Task 一个 commit；任一 Task 失败可直接 `git revert`。
- **不在本 plan 范围**：
  - 实现 `credential-login-service` 业务（仍是 stub）— 由独立 plan 负责
  - 重写 SDK 调新端口 — 端口未变（4000/6000/8081/8082/8083 沿用），SDK 无需改
  - 文档同步（除归档头）— README 仍提到 Go 需另行清理，但**不**阻塞本 plan
- **风险点**：
  - Task 5 Step 2 删 `auth-service` 块后，docker-compose 部署无法启动该服务；部署侧需在 `credential-login-service` 实施完后补回。

## 已知问题（plan 评审发现，不在本 plan 修）

| # | 问题 | 文件 | 影响 | 应在哪个 plan 修 |
|---|---|---|---|---|
| 1 | `services/credential-login-service/Cargo.toml` 只声明 `[package]`，无 `[[bin]]` 或 `src/main.rs`；`Dockerfile.credential-login-service` 用 `cargo build --release -p credential-login-service` 产二进制 → 实际只产 lib，build 失败 | `services/credential-login-service/Cargo.toml` | `docker build` 时 LNK / "no bin to build" | credential-login-service 业务 plan |
| 2 | `docker/docker-compose.yml` 中 `user-service.ports: "6000:6000"`，但 `services/user-service/config/config.yaml` 是 `port: 8082`，`Dockerfile.user-service` 是 `EXPOSE 8082` | `docker/docker-compose.yml` | `docker compose up` 后端口 6000 无人监听，healthcheck 必失败 | 独立 compose 修复 plan |
| 3 | `AGENTS.md` 把 `auth-service` / `user-service` 仍标为 Go + Gorilla/Mux，token-server 路径未更新 | `AGENTS.md` | 误导 AI 协作 | 文档同步 plan（与 README 一起）|
| 4 | `package.json` 字段 `"private": true` 仍包含在 workspace 中，与 pnpm-workspace 实际关系不明 | `package.json` | 无直接影响 | 评估 plan |

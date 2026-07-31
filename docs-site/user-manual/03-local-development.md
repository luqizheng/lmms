# 03. 本地开发

> 日常开发循环：起基础设施 → 起服务 → 改代码 → 测试 → 提交。

---

## 3.1 本地起服务（不用 docker 跑服务本身）

```bash
# 终端 1：基础设施
docker compose -f infrastructure/docker-compose.yml up -d

# 终端 2~9：各服务（并行）
for svc in token-service iam-service notification-service audit-service config-service file-service prue_backend; do
  (cd services/$svc && cargo run) &
done

# 或者用 pnpm 并行
pnpm run dev
```

api-gateway 与可观测性栈通常保留在 docker compose 里。

---

## 3.2 热重载 api-gateway 路由

```bash
# 编辑 infrastructure/config/api-gateway/routes/*.router.toml
# 二选一：
kill -HUP $(pgrep api-gateway)              # SIGHUP
# 或调 admin 接口：
curl -X POST http://localhost:3000/__admin/reload
```

---

## 3.3 VSCode 调试

`.vscode/launch.json` 示例：

```json
{
  "type": "lldb",
  "request": "launch",
  "name": "iam-service",
  "cargo": { "args": ["build", "--bin=iam-service"] },
  "args": [],
  "cwd": "${workspaceFolder}/services/iam-service"
}
```

先 `cargo build` 一次生成调试符号，再 F5 起服务。

---

## 3.4 给服务加依赖

```bash
# 1. 在 services/<svc>/Cargo.toml 加 dep
# 2. 用容器测试编译（避免本地 toolchain 问题）
node scripts/run-tests-docker.js iam-service
```

> **依赖升级 / 跨服务升级**走 `cargo upgrade -p <lib>`，详见 skill `.trae/skills/coder-lib-upgrade`。

---

## 3.5 改契约（错误码 / 权限码）

`contracts/` 是前后端单一真相源（`error-codes.yaml` / `permissions.yaml` / `events.yaml`）。

```bash
# 1. 改 contracts/error-codes.yaml 或 contracts/permissions.yaml
# 2. 重新生成 Rust 消费代码
node scripts/contracts-gen.js

# 3. CI 门禁：检查生成代码与 YAML 是否漂移
node scripts/contracts-gen.js --check
```

> **不要手改** `libs/digua_common-error/src/codes.rs` 与 `libs/digua_authz/src/perms.rs` —— 它们是生成产物。

---

## 3.6 数据库迁移

新加表 / 加列走 `V{N+1}__*.sql`。prue_backend 必须同时出 MySQL + SQLite 两版（`migrations/{mysql,sqlite}/`），其余服务只 MySQL。

详见 skill `.agents/skills/coder-migrate-db`。

---

## 3.7 调试技巧速查

| 现象 | 怎么做 |
|------|--------|
| 想看请求链路 | 见 [05 可观测性](./05-observability.md) § 查 trace |
| 想看 sql | 打开 sqlx 的 `LOG_STATEMENTS` env |
| 想清数据重来 | 见 [10 故障排查](./10-troubleshooting.md) § 整体重置 |
| 只想跑一个测试 | `cargo test --test <name> -- --nocapture <pattern>` |

---

## 3.8 推荐工作目录布局

每个开发者本地建议：

```
~/work/
  coder-micro-services/       ← 主仓
  ~/.cargo/registry/          ← 共享 registry 缓存（容器 / 本机共用）
  ~/tmp/coder-logs/           ← 日志收集点（可选）
```

# iam-service 容器内密码重置 CLI（iam_reset_password）设计

- 日期：2026-11-24
- 状态：已实施
- 关联：`services/iam-service/`、`docs/superpowers/specs/2026-10-27-host-native-build-test-design.md`（构建环境）

## 背景

管理员账号被锁死（密码遗忘 / 邮件自服务通道不可用）时，需要一个不依赖网关、
邮件、Redis 的运维兜底入口。方案：在 iam-service 镜像内附带一个 CLI 二进制，
**必须 `docker exec` 进入部署了 iam-service 的容器才能执行**——入口控制等价于
宿主机访问权，不新增网络攻击面。

镜像内多二进制是本仓既有模式（`iam_service` + `iam_outbox_publisher` 同镜像）。

## 目标 / 非目标

**目标**

1. 新增 `[[bin]] iam_reset_password`（`src/bin/reset-password.rs`），随 iam-service 镜像发布。
2. 用法：`iam_reset_password <username> [--password <pw>]`；省略 `--password` 时生成 24 位随机密码并打印（避免密码进 shell history）。
3. DB 语义与 `SelfServiceSvc::reset_password` 完全一致：
   - `UPDATE iam_users SET password_hash = ?, password_expired_at = NULL, updated_at = NOW() WHERE id = ?`（argon2 哈希复用 `digua_common_crypto::password::hash`）
   - 撤销全部会话：`UPDATE iam_refresh_tokens SET revoked = TRUE ... WHERE user_id = ?`
4. 连接零新增配置：直接读容器内已注入的 `DATABASE_URL`，经 `digua_common_db::init_pool_with` 建池；无需 Redis / RabbitMQ / 网关。

**非目标**

- 不新增 `contracts/events.yaml` 事件、不入站箱审计（与现有 reset/change 流程一致——它们同样不发审计事件；如需审计链路属后续独立任务）。
- 不做交互式 TUI / 批量重置。
- 不 bump 版本号（发镜像沿用现有 tag）。

## 方案要点

| 项 | 决策 | 理由 |
| --- | --- | --- |
| 二进制名 | `iam_reset_password`（全下划线） | 磁盘产物与 exec 名一致，规避 kebab→underscore 强制转换坑（AGENTS.md「Cargo [[bin]] 强制映射」节） |
| 用户定位 | `WHERE username = ? AND deleted_at IS NULL` | `iam_users.username` 全局 UNIQUE（V001），单用户名查找安全 |
| 方言 | 复用 `db_execute!` / `db_fetch_optional!` 宏 | 与服务代码同一套方言分发，PG 变体镜像无需改动 |
| 密码下限 | `--password` 时 ≥ 8 字符 | 运维工具防呆；生成路径（nanoid 24 位）恒满足 |
| access JWT | 不主动拉黑 | 与服务内改密码流程同语义：仅撤销 refresh token，短期 access token 到 exp 自然失效 |

## 变更清单

1. `services/iam-service/src/bin/reset-password.rs`（新增）：CLI + 纯单测（参数解析、随机密码长度/字符集）。
2. `services/iam-service/Cargo.toml`：追加 `[[bin]]`。
3. `services/iam-service/Dockerfile`：builder 阶段同一条 `cargo build` 增加 `--bin iam_reset_password`，runtime 阶段追加 `COPY`。
4. `services/iam-service/AGENTS.md` / `README.md`：补用法文档。

## 测试

- 纯单测（host 可跑，不依赖 MySQL）：`cargo test -p iam_service --bin iam_reset_password`。
- 编译验证：`node scripts/build-cargo.js --debug iam-service`。
- 门禁：无路由/OpenAPI/契约变更，跑 `check-service-standard.js` + openapi drift 确认无漂移。

## 发布

版本不变，仅重推 `micro-server/iam-service:<现版本>` 镜像；下游 `docker compose pull && up -d` 后即可 `docker exec -it <iam 容器> iam_reset_password <username>`。

## 安全考量

- 使用前提 = 宿主机 root 权限，等价现有运维信任边界，不扩攻击面。
- 输出仅打印生成的密码到 stdout（exec 会话内），不留文件；`--password` 直传会在 shell history 留痕，help 文案中提示优先省略。
- CLI 不监听端口、不注册路由，Swagger/OpenAPI 无感知。

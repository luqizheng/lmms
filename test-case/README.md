# 手工测试场景 — Phase 4.x 事件总线

本目录是 Phase 4.x（异步事件总线）四个小迭代的**手工测试场景**，供人工逐步执行验收。
自动化测试见 `tests/e2e/`（API）与各服务 `tests/`（testcontainers 集成）；本目录补充
**跨服务、事件驱动、需要观察 RabbitMQ/Redis/DB 副作用**的端到端场景。

## 覆盖范围

| 文件 | Phase | 主题 |
|------|-------|------|
| `00-setup.md` | — | 起栈、拿 admin token、通用工具命令 |
| `41-notification-welcome.md` | 4.1 | 新建用户 → 欢迎站内信 |
| `42-login-failed-risk.md` | 4.2 | 登录失败事件 + 风控滑动窗口告警 |
| `43-announcement-ws.md` | 4.3 | 公告发布 → 事件驱动 WebSocket 广播（多副本） |
| `44-permission-epoch.md` | 4.4 | 权限变更 → 令牌失效（perm-epoch 双写） |

## 执行约定

- 每个场景文件是一张**步骤表**：`步骤 / 操作 / 期望结果 / 实测 / PASS·FAIL`。
- 先做 `00-setup.md` 把栈起来、导出 `$TOKEN` / `$BASE` 环境变量，再按 4.1 → 4.4 顺序执行。
- 标 ⚠️ 的步骤涉及破坏性操作（停容器、改权限），在隔离环境执行。
- 命令以 `curl` + `docker exec` 为主，无需额外工具；可选 `jq` 美化 JSON。

## 服务端口速查（compose 默认）

| 组件 | 地址 | 备注 |
|------|------|------|
| api-gateway | `http://localhost:3000` | 对外入口，所有 `/v1/**` 走它 |
| iam-service | `http://localhost:6002` | 直连（绕过 gateway 调试用） |
| notification-service | `http://localhost:6003` | WS: `/notification/ws` |
| audit-service | `http://localhost:6005` | |
| MySQL | `localhost:3306` | root / rootpass / `superpowers` |
| Redis | `localhost:6379` | perm_epoch / 风控窗口 / 黑名单 |
| RabbitMQ 管理台 | `http://localhost:15672` | guest / guest |

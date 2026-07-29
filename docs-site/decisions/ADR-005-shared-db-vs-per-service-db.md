# ADR-005: Shared-DB vs Per-Service-DB 架构评估

> 日期：2026-07-29
> 状态：已采纳
> 关联：
> - [ADR-003 多租户架构](./ADR-003-multi-tenancy-architecture.md)
> - [DB 命名规范契约](../../contracts/db-conventions.md)

## 上下文

平台当前 8 个服务（iam、token、notification、audit、config、file、prue_backend、api-gateway 不直连 DB）共享同一个 MySQL 库 `superpowers`，通过**表名前缀**做逻辑隔离（`iam_` / `audit_` / `token_` / `ntf_` / `cfg_` / `file_` / `prue_`），CI gate（`check-db-naming.js`）强制表名必须带服务前缀。

pre-v1 架构评审（2026-07-29）指出：逻辑隔离 ≠ 物理隔离，任一服务写坏共享库会导致全平台挂。需要一份明确的决策记录：当前路径是刻意选择还是路径依赖？短期和长期路线是什么？

## 决议

### 1.0 阶段（当前）：维持共享 DB + 强化隔离

**决定**：共享 DB 路径继续保留，但补两层加固：

1. **每服务独立 DB user + 表级权限**（零迁移成本）：
   - `iam_svc` 用户仅 `SELECT/INSERT/UPDATE/DELETE` on `iam_*` 表
   - `audit_svc` 用户仅对 `audit_*` 表有权限
   - 即使代码写错 `INSERT INTO iam_users`，在 audit-service 连接上也会被 MySQL 拒绝
   - 迁移脚本在 `infrastructure/mysql/init/` 中初始化这些用户

2. **`check-tenant-scope.js` 治理率跟踪**：
   - 当前 delegated 豁免项统计（Phase 8.3-fu2 提到已有豁免）
   - 目标：1.0 结束前 delegated 豁免率 < 10%

### 2.0 阶段（post-1.0）：每服务独立 DB（Per-Service-DB）

**规划**：当 K3s 化部署上线后，按需拆分 DB。

| 服务 | 当前表前缀 | 2.0 独立 DB | 优先级 |
|------|----------|------------|--------|
| iam-service | `iam_*` | `superpowers_iam` | 高（认证核心） |
| token-service | `token_*` | `superpowers_token` | 高（令牌核心） |
| audit-service | `audit_*` | `superpowers_audit` | 中（日志独立） |
| notification-service | `ntf_*` | `superpowers_ntf` | 低（非关键） |
| config-service | `cfg_*` | `superpowers_cfg` | 低（只读为主） |
| file-service | `file_*` | `superpowers_file` | 中（大对象） |
| prue_backend | `prue_*` | `superpowers_prue` | 中（流程引擎） |

**拆分策略**：逐服务迁移，用新表前缀 + 新 DB user，老表保留不删（回滚路径）。

### 为什么不现在拆

| 因素 | 评估 |
|------|------|
| 部署复杂度 | 当前 docker-compose 单 MySQL 容器，拆 DB = 8 个独立容器或 8 个 DB，运维成本翻倍 |
| 迁移风险 | 8 个服务同时改 DB 连接字符串，一次全停机风险高 |
| 实际收益 | 当前流量下，单服务写坏 DB 的概率极低（开发阶段，非生产流量） |
| CI 现有保护 | `check-db-naming.js` + table prefix 已在代码层强隔离 |

## 后果

### 好处

- **1.0 交付不受阻**：不增加部署运维复杂度
- **DB user 权限加固**：零迁移成本加一道安全网
- **明确 2.0 路线**：有清晰拆分路径和时间点

### 代价

- 物理上仍是共享库，单实例故障影响全平台（MySQL 主从 P0-4 已覆盖灾备）
- 迁移到 Per-Service-DB 时需处理跨库查询（目前跨服务走 RabbitMQ + outbox，无跨库 JOIN）
- 2.0 拆分需要投入 ≈ 1 周迁移 + 测试

### 已知未做

- 数据库连接池隔离（当前所有服务共享同一 JDBC/连接配置模板）
- 慢查询按服务来源标记（可在 common-db 中加 `application_name` 到连接参数）

## 变更时机

- **新增服务**：默认走共享 DB + 独立 DB user（遵循本 ADR §1.0）
- **2.0 启动信号**：K3s 部署就绪 + 出现第一个生产级流量场景
- **拆分先拆哪个**：按本 ADR 优先级表，从 iam-service / token-service 开始

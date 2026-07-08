# Schema State 端点 — 运维操作指南

> **状态**：v1 · Phase 10 G3 落地
> **关联**：spec `docs/superpowers/specs/2026-07-08-migrations-state-endpoint-design.md`
> **覆盖**：7 个 Rust 服务（iam / notification / token / audit / config / file / prue_backend）

---

## 1. 一句话

> 每个 Rust 服务暴露 `GET /<svc>/internal/migrations/state`，返回该服务的 DB schema 当前状态（applied / available / in_sync / missing / extra）。运维 / 排障 / schema drift 检测的统一入口。

---

## 2. 端点清单

| 服务 | 路径 | Migration 数 | dialect |
|------|------|-------------|---------|
| iam-service | `GET /iam/internal/migrations/state` | 5 (V001-V005) | mysql |
| notification-service | `GET /ntf/internal/migrations/state` | 3 (V001-V003) | mysql |
| token-service | `GET /token/internal/migrations/state` | 1 (V001) | mysql |
| audit-service | `GET /audit/internal/migrations/state` | 1 (V001) | mysql |
| config-service | `GET /cfg/internal/migrations/state` | 2 (V001-V002) | mysql |
| file-service | `GET /file/internal/migrations/state` | 2 (V001-V002) | mysql |
| prue_backend | `GET /prue/internal/migrations/state` | 1 (V001) × 2 dialects | mysql+sqlite |

---

## 3. 调用示例

### 3.1 单一服务

```bash
curl -s http://iam-service:8081/iam/internal/migrations/state | jq .
```

正常响应（所有 migration 已应用）：

```json
{
  "service": "iam-service",
  "dialect": "mysql",
  "available": [
    { "version": "V001", "name": "init" },
    { "version": "V002", "name": "outbox_archive" },
    { "version": "V003", "name": "tenants" },
    { "version": "V004", "name": "tenant_business_tables" },
    { "version": "V005", "name": "platform_admin_role" }
  ],
  "applied": ["V001", "V002", "V003", "V004", "V005"],
  "current_version": "V005",
  "in_sync": true,
  "missing": [],
  "extra": []
}
```

### 3.2 DB 不可达

```bash
curl -s -i http://iam-service:8081/iam/internal/migrations/state | head -5
# HTTP/1.1 503 Service Unavailable
# {"service":"iam-service","error":"db_unreachable","detail":"connection refused"}
```

### 3.3 跨服务联邦检查

```bash
# 一键查所有 7 个服务的 schema 状态（需 docker compose 已起）
for url in \
  http://localhost:8081/iam/internal/migrations/state \
  http://localhost:8083/ntf/internal/migrations/state \
  http://localhost:8082/token/internal/migrations/state \
  http://localhost:8084/audit/internal/migrations/state \
  http://localhost:8085/cfg/internal/migrations/state \
  http://localhost:8086/file/internal/migrations/state \
  http://localhost:8087/prue/internal/migrations/state ; do
  echo "=== $url ==="
  curl -fsS "$url" | jq '{service, in_sync, current_version, missing, extra}' || echo "DOWN"
done
```

---

## 4. 字段语义

| 字段 | 类型 | 含义 |
|------|------|------|
| `service` | string | 服务名，例 `"iam-service"` |
| `dialect` | string | `"mysql"` / `"sqlite"` / `"mysql+sqlite"`（prue_backend 双 dialect） |
| `available` | array | 该服务 `migrations/<dialect>/V00X__*.sql` 目录里所有 migration |
| `applied` | array | 通过 marker table 探测出的"已应用"版本（升序、去重） |
| `current_version` | string \| null | 最大的 applied 版本（含 extra 部分）；若 applied 为空则为 null |
| `in_sync` | bool | `true` 当 `missing` 与 `extra` 都为空 |
| `missing` | array | available 中未 applied 的（**升级时这里非空**） |
| `extra` | array | applied 中不在 available 列表的（**理想为空**；出现意味着 DB 多于代码声明） |

---

## 5. 典型场景

### 5.1 升级前确认

新代码带 V006 migration，升级前检查 DB 当前状态：

```bash
curl -fsS http://iam-service:8081/iam/internal/migrations/state | jq '.current_version'
# 期望: "V005"
```

### 5.2 升级后验证

升级完成 → 重启服务 → 调用端点确认 `in_sync: true` 且 `current_version: "V006"`。

### 5.3 schema drift 检测

```bash
curl -fsS http://iam-service:8081/iam/internal/migrations/state | jq '.missing'
# 期望: []
# 非空: 升级未跑完，或代码回滚但 DB 未跟着回滚
```

### 5.4 rollback 决策

回滚 V006 时，确认目标版本：

```bash
curl -fsS http://iam-service:8081/iam/internal/migrations/state | jq '.applied | .[-2]'
# 期望: "V005"（rollback 的目标）
```

### 5.5 prue_backend 双 dialect 一致性

prue_backend 同时跑 MySQL + SQLite 两份 DB，确认两边都同步：

```bash
curl -fsS http://prue-backend:8087/prue/internal/migrations/state | jq '{service, dialect, applied, in_sync}'
# 期望: { "service": "prue_backend", "dialect": "mysql+sqlite",
#         "applied": ["V001"], "in_sync": true }
# 注意: prue_backend 只回一个 endpoint，但内部会根据 DbPool 类型探测对应 dialect 的 marker table
```

---

## 6. 鉴权

### 6.1 当前实现（Phase 10 G3 现状）

| 服务 | 鉴权层 |
|------|--------|
| iam-service | service-jwt `aud=iam-service`（已接 `verify_aud_principal_middleware`）|
| 其他 6 服务 | **无鉴权层**（依赖内网 / 网络层隔离）|

### 6.2 风险与缓解

无鉴权意味着任何能访问到端点 URL 的内部服务/人都能读 schema 状态。schema 状态本身不含敏感数据（表名 + 版本号），但仍是内部信息。

**生产部署前必须补的**：
1. 在 6 个服务接入 service-jwt 鉴权（每个服务 ~半天，与 Phase 5-B 跨面扩面合并做）
2. 或：网络层隔离（仅运维子网可达）

### 6.3 触发条件

任一即重开鉴权扩面工作：
- 真实生产部署
- 安全审计 / 等保合规需求
- 内部网络隔离失效（flat network）

---

## 7. CI 集成（建议）

### 7.1 dev 环境快速验证

`scripts/check-migration-state.sh`（建议新增）：

```bash
#!/usr/bin/env bash
# 假设 docker compose 启了所有服务
set -e
URLS=(
  "http://localhost:8081/iam/internal/migrations/state"
  "http://localhost:8083/ntf/internal/migrations/state"
  ...
)
for url in "${URLS[@]}"; do
  body=$(curl -fsS "$url")
  in_sync=$(echo "$body" | jq -r '.in_sync')
  if [ "$in_sync" != "true" ]; then
    echo "DRIFT: $url → $body"
    exit 1
  fi
done
echo "all 7 services in_sync"
```

### 7.2 GitLab CI 接入

在 `test` 阶段 docker compose up 完整 stack → 跑脚本 → 任一服务 drift 即 fail。

---

## 8. 实现细节（开发者参考）

- **抽象层**：`libs/common-db::MigrationState` trait + `build_response()`
- **probe helper**：`probe_marker_mysql()` / `probe_marker_sqlite()`（查 information_schema.tables / sqlite_master）
- **服务接入**：每个服务实现 `MigrationState` + 写 thin handler（~70 行）
- **依赖**：`common-db = { ..., features = ["mysql", "sqlite"] }`（prue_backend）/ `["mysql"]`（其他）

详见各服务 commit 与 spec。

---

## 9. 关联

- `docs/superpowers/specs/2026-07-08-migrations-state-endpoint-design.md` — 设计
- `docs/superpowers/specs/2026-07-08-prue-backend-sqlite-decision-design.md` — prue_backend 双 dialect 决策
- TODO.md §Phase 10 G3 — 任务跟踪
- 各服务 AGENTS.md — 端点域条目
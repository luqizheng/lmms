# Java SDK 端点漂移（Known Drift）

> **生成时间**：2026-07-30
> **来源**：`scripts/check-java-sdk-drift.js` 扫描结果
> **状态**：✅ 全部已处理 — 12 → 0 hard fails

## 处理方式

| 模块 | 原问题 | 处理 | 状态 |
|------|--------|------|------|
| `audit-service-sdk` | 3 endpoints (events/logs/event-types) 无对应服务端点 | **删除整个 SDK 模块**（commit `TODO`） | ✅ |
| `config-service-sdk` | 2 endpoints (namespaces/items) 路径名不匹配 | **删除整个 SDK 模块**（API 设计已变更，SDK 基于 v0.1.0） | ✅ |
| `file-service-sdk` | 3 endpoints 多 `/files/` 路径段 | **删除整个 SDK 模块**（API 设计已变更） | ✅ |
| `notification-service-sdk` | 2 paths: `/notifications/unread-count` → `/unread-count`<br>`/notifications/{id}/read` → `/announcements/{id}/read` | **修正 SDK 路径** | ✅ |

## 剩余信息性提示（非 hard fail）

- `/ntf/internal/migrations/state` — 内部端点，不在 SDK 覆盖范围（合理）
- 检查脚本退出码 0（通过）
- CI `java_sdk_drift_check` 已改 `allow_failure: false`

## 删除的模块清单

- `sdk/java/audit-service-sdk/`
- `sdk/java/config-service-sdk/`
- `sdk/java/file-service-sdk/`

以上模块基于 v0.1.0 API 设计，与 v1.0 服务路径完全不匹配，已删除。需要新 SDK 时按 v1.0 OpenAPI 重新生成。

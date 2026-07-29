# Java SDK 端点漂移（Known Drift）

> **生成时间**：2026-07-29
> **来源**：`scripts/check-java-sdk-drift.js` 扫描结果
> **状态**：P2 技术债 — 不阻塞 1.0，但需在 1.0 前或 1.0 后逐项修复

## 背景

Java SDK 在 `v0.2.0` 完成了 v1 路由迁移（`/v1/iam/...`、`/v1/tokens/...`），但 Phase 9.5 spec 已承认部分 Java SDK 端点在现网服务中不存在。本文档列出 `check-java-sdk-drift.js` 检测到的硬失败项，供后续修复排期。

> **校验依据**：Rust SDK 以 `services/<svc>/openapi.yaml` 为准（CI gate `dump-openapi-specs.sh` 强制同步）。Java SDK 应同步对齐。

## 硬失败（12 项 · SDK 调用的路径在 OpenAPI 中不存在）

| SDK 模块 | SDK 调用路径 | OpenAPI 实际路径 | 服务 |
|----------|------------|------------------|------|
| `audit-service-sdk` | `/v1/audit/events` | `/audit/operation` | audit-service |
| `audit-service-sdk` | `/v1/audit/logs` | `/audit/login` | audit-service |
| `audit-service-sdk` | `/v1/audit/event-types` | （缺失） | audit-service |
| `config-service-sdk` | `/v1/config/namespaces` | （缺失） | config-service |
| `config-service-sdk` | `/v1/config/items/{id}` | （缺失） | config-service |
| `file-service-sdk` | `/v1/file/files/presign` | （缺失） | file-service |
| `file-service-sdk` | `/v1/file/files/upload` | （缺失） | file-service |
| `file-service-sdk` | `/v1/file/files/{id}` | （缺失） | file-service |
| `notification-service-sdk` | `/v1/notification/announcements` | （缺失） | notification-service |
| `notification-service-sdk` | `/v1/notification/notifications/unread-count` | （缺失） | notification-service |
| `notification-service-sdk` | `/v1/notification/notifications/{id}/read` | （缺失） | notification-service |

## 修复方向（待评审）

每个端点需要判断走哪条路：

### 方向 A：服务侧补端点（推荐）

适用：API 语义有价值、只是 OpenAPI/spec 漏写。

- 检查 service 实际 handler 是否实现
- 补充 `#[utoipa::path(...)]` 注解
- 跑 `scripts/dump-openapi-specs.sh` 重新生成 OpenAPI
- 验证 SDK 调用与新生成的 OpenAPI 一致

### 方向 B：SDK 侧改路径

适用：服务侧没有该端点、SDK 是 v0.1.0 残留。

- 找到 SDK 中调用的方法
- 改为调用服务侧真实存在的路径
- SDK 同步版本号（0.2.0 → 0.3.0）

### 方向 C：废弃 SDK 端点

适用：服务功能下线、SDK 不再需要。

- SDK 方法标 `@Deprecated`
- 增加日志提醒
- 下一 major 版本删除

## 优先修复顺序

| 优先级 | 项 | 理由 |
|--------|---|------|
| P1 | audit `/v1/audit/events` → `/audit/operation` | SDK 核心日志接口，影响所有调用方 |
| P1 | audit `/v1/audit/logs` → `/audit/login` | 同上 |
| P2 | config namespaces/items | ConfigServiceClient 全部调用都受影响 |
| P2 | file presign/upload | FileServiceClient 核心功能 |
| P2 | notification announcements/notifications | 公告核心 |
| P3 | audit `/v1/audit/event-types` | 辅助端点，可暂时返回空数组 |
| P3 | notification unread-count | UI 优化项，不阻塞核心流 |

## CI 处理

`java_sdk_drift_check` job 当前 `allow_failure: true`（2026-07-29 设置）—— 脚本报告但不阻断 pipeline，避免现有 drift 阻塞 SDK 维护工作流。

```yaml
java_sdk_drift_check:
  stage: test
  allow_failure: true  # 已知 12 处 drift，不阻断；修复后改 false
  ...
```

## 验证步骤（修复时）

1. 改 SDK 或 OpenAPI
2. 跑 `node scripts/check-java-sdk-drift.js`，确保 12 项全部清零
3. 跑 SDK 单测 `node scripts/build-java-sdk-docker.js test`
4. 提交 PR 时附 `check-java-sdk-drift.js` 输出 diff
5. CI 中 `java_sdk_drift_check` 改 `allow_failure: false`
6. 更新本文档"硬失败"表格为空（标记完成）
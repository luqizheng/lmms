# audit-services

lmms audit-service SDK：**登录/操作日志查询、合规导出（CSV/JSON）、保留策略管理**的 TypeScript 客户端。零运行时依赖（原生 `fetch`），浏览器与 Node ≥18 通用，面向 api-gateway 的 `/v1/audit` 路径。

与 `iam-services` 同款模式（SDK 位置例外见 `sdk/README.md`）；安装与 `.npmrc` 配置见该包 README。

## 快速上手

```ts
import { createAuditServices } from "@digua/audit-services";

const audit = createAuditServices({
  baseUrl: "http://127.0.0.1:3000", // 网关；浏览器同域反代可传相对路径
  getToken: () => localStorage.getItem("access_token") ?? undefined,
});

// 日志查询（分页，需鉴权）
const logs = await audit.logs.queryLogin({ username: "admin", success: false, page: 1, size: 20 });
const ops = await audit.logs.queryOperation({ user_id: "usr-1", resource: "project" });

// 合规导出（文本/数组）
const csv = await audit.exports.loginCsv("usr-1");           // text/csv 字符串
const json = await audit.exports.operationJson("usr-1");     // OperationLog[]

// 保留策略
await audit.retention.updatePolicy({ hot_days: 7, warm_days: 30, cold_days: 90, archive_enabled: true, purge_enabled: false });
const stats = await audit.retention.stats();
```

## API 一览

| 模块 | 方法 | 端点 |
|---|---|---|
| `logs` | `queryLogin` / `queryOperation` | `GET /v1/audit/login`、`GET /v1/audit/operation`（分页） |
| `exports` | `loginCsv` / `operationCsv` / `operationJson` | `GET /v1/audit/export/compliance/...`（CSV 返回文本，JSON 返回裸数组） |
| `retention` | `getPolicy` / `updatePolicy` / `stats` / `cleanup` / `dataStage` | `GET/PUT /v1/audit/retention/policy` 等 |

## 错误模型

统一抛 `AuditServicesError`（`status` / `code` / `message` / `requestId` / `body`）。audit-service 走 common-error 的 `{code,message,request_id}`（如 `AUD_LOG_1001`）；网关层错误为纯文本，两种形态都归一。

## 陷阱

1. `updatePolicy` 违反 hot<warm<cold 返回 400（错误码沿用 `CFG_VALID_2003/2004/2005`）。
2. `cleanup` / `dataStage` 的响应形状未建模进 openapi（YAML 为 `{}`），SDK 按处理器实际输出镜像。
3. 保留策略相关端点全部需要鉴权；`/v1/audit/health` 是唯一的公开路由。

## 开发

```bash
pnpm --filter @digua/audit-services test / typecheck / build
```

发布走仓库统一管线：`pnpm run packages:check`（干跑）/ `pnpm run packages:publish --version <x.y.z>`，规则见 `AGENTS.md`「npm 包发布规则」。

// 日志查询（audit-service，经网关 /v1/audit/login|operation，全部需要鉴权）。
// 路由事实源：services/audit-service/src/routes/。

import type { HttpCore } from "./http.js";
import type {
  LoginLogQuery,
  LoginLog,
  OperationLog,
  OperationLogQuery,
  PagedLoginLogs,
  PagedOperationLogs,
} from "./types.js";

export class AuditLogsApi {
  constructor(private readonly http: HttpCore) {}

  /** 分页查询登录日志：GET /v1/audit/login。 */
  queryLogin(query: LoginLogQuery = {}): Promise<PagedLoginLogs> {
    return this.http.get<PagedLoginLogs>("/v1/audit/login", { params: query, auth: true });
  }

  /** 分页查询操作日志：GET /v1/audit/operation。 */
  queryOperation(query: OperationLogQuery = {}): Promise<PagedOperationLogs> {
    return this.http.get<PagedOperationLogs>("/v1/audit/operation", { params: query, auth: true });
  }
}

export type { LoginLog, OperationLog };

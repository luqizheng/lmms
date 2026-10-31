import type { HttpCore } from "./http.js";
import type { LoginLogQuery, LoginLog, OperationLog, OperationLogQuery, PagedLoginLogs, PagedOperationLogs } from "./types.js";
export declare class AuditLogsApi {
    private readonly http;
    constructor(http: HttpCore);
    /** 分页查询登录日志：GET /v1/audit/login。 */
    queryLogin(query?: LoginLogQuery): Promise<PagedLoginLogs>;
    /** 分页查询操作日志：GET /v1/audit/operation。 */
    queryOperation(query?: OperationLogQuery): Promise<PagedOperationLogs>;
}
export type { LoginLog, OperationLog };
//# sourceMappingURL=logs.d.ts.map
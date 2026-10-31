import type { HttpCore } from "./http.js";
import type { OperationLog } from "./types.js";
export declare class AuditExportsApi {
    private readonly http;
    constructor(http: HttpCore);
    /** 按用户导出登录日志 CSV：GET /v1/audit/export/compliance/login/csv/{user_id}，返回文本。 */
    loginCsv(userId: string): Promise<string>;
    /** 按用户导出操作日志 CSV：GET /v1/audit/export/compliance/operation/csv/{user_id}，返回文本。 */
    operationCsv(userId: string): Promise<string>;
    /** 按用户导出操作日志 JSON：GET /v1/audit/export/compliance/operation/json/{user_id}，返回裸数组。 */
    operationJson(userId: string): Promise<OperationLog[]>;
}
//# sourceMappingURL=exports.d.ts.map
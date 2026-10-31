import { type AuditServicesOptions } from "./http.js";
import { AuditExportsApi } from "./exports.js";
import { AuditLogsApi } from "./logs.js";
import { AuditRetentionApi } from "./retention.js";
export * from "./types.js";
export * from "./errors.js";
export { DEFAULT_BASE_URL, HttpCore, parseDispositionFilename, type AuditServicesOptions, type RequestOptions, type BytesResponse } from "./http.js";
export { AuditLogsApi } from "./logs.js";
export { AuditExportsApi } from "./exports.js";
export { AuditRetentionApi } from "./retention.js";
export interface AuditServices {
    /** 登录/操作日志分页查询。 */
    readonly logs: AuditLogsApi;
    /** 合规导出（CSV/JSON）。 */
    readonly exports: AuditExportsApi;
    /** 保留策略与统计。 */
    readonly retention: AuditRetentionApi;
}
/** 创建 audit-service 客户端。 */
export declare function createAuditServices(options?: AuditServicesOptions): AuditServices;
//# sourceMappingURL=index.d.ts.map
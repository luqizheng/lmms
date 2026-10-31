// audit-services：lmms audit-service SDK（日志查询 / 合规导出 / 保留策略）。
// 零运行时依赖，原生 fetch，浏览器 / Node ≥18 通用；面向 api-gateway 的 /v1/audit 公开路径。
import { HttpCore } from "./http.js";
import { AuditExportsApi } from "./exports.js";
import { AuditLogsApi } from "./logs.js";
import { AuditRetentionApi } from "./retention.js";
export * from "./types.js";
export * from "./errors.js";
export { DEFAULT_BASE_URL, HttpCore, parseDispositionFilename } from "./http.js";
export { AuditLogsApi } from "./logs.js";
export { AuditExportsApi } from "./exports.js";
export { AuditRetentionApi } from "./retention.js";
/** 创建 audit-service 客户端。 */
export function createAuditServices(options = {}) {
    const http = new HttpCore(options);
    return {
        logs: new AuditLogsApi(http),
        exports: new AuditExportsApi(http),
        retention: new AuditRetentionApi(http),
    };
}
//# sourceMappingURL=index.js.map
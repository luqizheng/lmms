// audit-services：lmms audit-service SDK（日志查询 / 合规导出 / 保留策略）。
// 零运行时依赖，原生 fetch，浏览器 / Node ≥18 通用；面向 api-gateway 的 /v1/audit 公开路径。

import { HttpCore, type AuditServicesOptions } from "./http.js";
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
export function createAuditServices(options: AuditServicesOptions = {}): AuditServices {
  const http = new HttpCore(options);
  return {
    logs: new AuditLogsApi(http),
    exports: new AuditExportsApi(http),
    retention: new AuditRetentionApi(http),
  };
}

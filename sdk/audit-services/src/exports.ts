// 合规导出（audit-service，经网关 /v1/audit/export/compliance/**，需要鉴权）。
// 注意：CSV 实际 Content-Type 为 text/csv（openapi 误标 text/plain）；
// JSON 导出实际返回 application/json 的裸数组。

import type { HttpCore } from "./http.js";
import type { OperationLog } from "./types.js";

export class AuditExportsApi {
  constructor(private readonly http: HttpCore) {}

  /** 按用户导出登录日志 CSV：GET /v1/audit/export/compliance/login/csv/{user_id}，返回文本。 */
  loginCsv(userId: string): Promise<string> {
    return this.http.get<string>(`/v1/audit/export/compliance/login/csv/${encodeURIComponent(userId)}`, {
      auth: true,
      response: "text",
    });
  }

  /** 按用户导出操作日志 CSV：GET /v1/audit/export/compliance/operation/csv/{user_id}，返回文本。 */
  operationCsv(userId: string): Promise<string> {
    return this.http.get<string>(`/v1/audit/export/compliance/operation/csv/${encodeURIComponent(userId)}`, {
      auth: true,
      response: "text",
    });
  }

  /** 按用户导出操作日志 JSON：GET /v1/audit/export/compliance/operation/json/{user_id}，返回裸数组。 */
  operationJson(userId: string): Promise<OperationLog[]> {
    return this.http.get<OperationLog[]>(
      `/v1/audit/export/compliance/operation/json/${encodeURIComponent(userId)}`,
      { auth: true }
    );
  }
}

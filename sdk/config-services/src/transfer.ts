// 配置导入/导出与监听（config-service，经网关 /v1/config/**，需要鉴权）。
// 注意：export/import 的请求与响应体都是 text/plain 承载的 JSON 对象映射（非 application/json）。

import type { HttpCore } from "./http.js";
import type { ImportResult, WatchResult } from "./types.js";

export class ConfigTransferApi {
  constructor(private readonly http: HttpCore) {}

  /**
   * 导出命名空间全部配置：GET /v1/config/export/{namespace_id}。
   * 返回 text/plain 承载的 JSON 对象映射 {key: value}（字符串），按需自行 JSON.parse。
   */
  exportConfig(namespaceId: string): Promise<string> {
    return this.http.get<string>(`/v1/config/export/${encodeURIComponent(namespaceId)}`, {
      auth: true,
      response: "text",
    });
  }

  /**
   * 导入配置（覆盖同名 key）：POST /v1/config/import/{namespace_id}。
   * 请求体为 text/plain 承载的 JSON 对象映射 {key: value}，返回 {success, count}。
   */
  importConfig(namespaceId: string, values: Record<string, unknown>): Promise<ImportResult> {
    return this.http.post<ImportResult>(`/v1/config/import/${encodeURIComponent(namespaceId)}`, JSON.stringify(values), {
      auth: true,
      contentType: "text/plain",
    });
  }

  /**
   * 配置变更监听（长轮询）：GET /v1/config/watch。
   * 注意：上游当前为 stub——睡眠 timeout 后恒返回 {changed:false, data:[]}，勿据此做推送。
   */
  watch(namespaceId?: string, timeoutSeconds?: number): Promise<WatchResult> {
    return this.http.get<WatchResult>("/v1/config/watch", {
      params: { namespace_id: namespaceId, timeout: timeoutSeconds },
      auth: true,
    });
  }
}

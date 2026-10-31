import type { HttpCore } from "./http.js";
import type { ImportResult, WatchResult } from "./types.js";
export declare class ConfigTransferApi {
    private readonly http;
    constructor(http: HttpCore);
    /**
     * 导出命名空间全部配置：GET /v1/config/export/{namespace_id}。
     * 返回 text/plain 承载的 JSON 对象映射 {key: value}（字符串），按需自行 JSON.parse。
     */
    exportConfig(namespaceId: string): Promise<string>;
    /**
     * 导入配置（覆盖同名 key）：POST /v1/config/import/{namespace_id}。
     * 请求体为 text/plain 承载的 JSON 对象映射 {key: value}，返回 {success, count}。
     */
    importConfig(namespaceId: string, values: Record<string, unknown>): Promise<ImportResult>;
    /**
     * 配置变更监听（长轮询）：GET /v1/config/watch。
     * 注意：上游当前为 stub——睡眠 timeout 后恒返回 {changed:false, data:[]}，勿据此做推送。
     */
    watch(namespaceId?: string, timeoutSeconds?: number): Promise<WatchResult>;
}
//# sourceMappingURL=transfer.d.ts.map
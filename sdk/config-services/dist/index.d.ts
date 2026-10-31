import { type ConfigServicesOptions } from "./http.js";
import { ConfigItemsApi } from "./items.js";
import { ConfigNamespacesApi } from "./namespaces.js";
import { ConfigTransferApi } from "./transfer.js";
export * from "./types.js";
export * from "./errors.js";
export { DEFAULT_BASE_URL, HttpCore, parseDispositionFilename, type ConfigServicesOptions, type RequestOptions, type BytesResponse } from "./http.js";
export { ConfigNamespacesApi } from "./namespaces.js";
export { ConfigItemsApi } from "./items.js";
export { ConfigTransferApi } from "./transfer.js";
export interface ConfigServices {
    /** 命名空间管理。 */
    readonly namespaces: ConfigNamespacesApi;
    /** 配置项管理（含秘密掩码 value_masked）。 */
    readonly items: ConfigItemsApi;
    /** 导入/导出（text/plain 承载 JSON 映射）与配置监听（上游为 stub）。 */
    readonly transfer: ConfigTransferApi;
}
/** 创建 config-service 客户端。 */
export declare function createConfigServices(options?: ConfigServicesOptions): ConfigServices;
//# sourceMappingURL=index.d.ts.map
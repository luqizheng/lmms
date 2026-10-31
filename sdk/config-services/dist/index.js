// config-services：lmms config-service SDK（命名空间 / 配置项 / 导入导出 / 监听）。
// 零运行时依赖，原生 fetch，浏览器 / Node ≥18 通用；面向 api-gateway 的 /v1/config 公开路径。
import { HttpCore } from "./http.js";
import { ConfigItemsApi } from "./items.js";
import { ConfigNamespacesApi } from "./namespaces.js";
import { ConfigTransferApi } from "./transfer.js";
export * from "./types.js";
export * from "./errors.js";
export { DEFAULT_BASE_URL, HttpCore, parseDispositionFilename } from "./http.js";
export { ConfigNamespacesApi } from "./namespaces.js";
export { ConfigItemsApi } from "./items.js";
export { ConfigTransferApi } from "./transfer.js";
/** 创建 config-service 客户端。 */
export function createConfigServices(options = {}) {
    const http = new HttpCore(options);
    return {
        namespaces: new ConfigNamespacesApi(http),
        items: new ConfigItemsApi(http),
        transfer: new ConfigTransferApi(http),
    };
}
//# sourceMappingURL=index.js.map
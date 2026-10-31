// config-services：lmms config-service SDK（命名空间 / 配置项 / 导入导出 / 监听）。
// 零运行时依赖，原生 fetch，浏览器 / Node ≥18 通用；面向 api-gateway 的 /v1/config 公开路径。

import { HttpCore, type ConfigServicesOptions } from "./http.js";
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
export function createConfigServices(options: ConfigServicesOptions = {}): ConfigServices {
  const http = new HttpCore(options);
  return {
    namespaces: new ConfigNamespacesApi(http),
    items: new ConfigItemsApi(http),
    transfer: new ConfigTransferApi(http),
  };
}

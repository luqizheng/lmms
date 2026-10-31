// prue-services：lmms prue_backend SDK（动态路由 + 菜单配置版本管理）。
// 零运行时依赖，原生 fetch，浏览器 / Node ≥18 通用；面向 api-gateway 的 /v1/pure 公开路径。
// 注意：本服务全响应包 {code,message,data} 信封，SDK 统一解包。

import { HttpCore, type PrueServicesOptions } from "./http.js";
import { PrueMenuConfigsApi, PrueRoutesApi } from "./configs.js";

export * from "./types.js";
export * from "./errors.js";
export * from "./envelope.js";
export { DEFAULT_BASE_URL, HttpCore, parseDispositionFilename, type PrueServicesOptions, type RequestOptions, type BytesResponse } from "./http.js";
export { PrueRoutesApi, PrueMenuConfigsApi } from "./configs.js";

export interface PrueServices {
  /** 动态路由（get-async-routes）。 */
  readonly routes: PrueRoutesApi;
  /** 菜单配置版本 CRUD / 克隆 / 发布（ETag 乐观锁）。 */
  readonly configs: PrueMenuConfigsApi;
}

/** 创建 prue_backend 客户端。 */
export function createPrueServices(options: PrueServicesOptions = {}): PrueServices {
  const http = new HttpCore(options);
  return {
    routes: new PrueRoutesApi(http),
    configs: new PrueMenuConfigsApi(http),
  };
}

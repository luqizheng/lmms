import { type PrueServicesOptions } from "./http.js";
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
export declare function createPrueServices(options?: PrueServicesOptions): PrueServices;
//# sourceMappingURL=index.d.ts.map
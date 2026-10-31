import type { HttpCore } from "./http.js";
import type { CreateMenuConfigRequest, MenuConfigDetail, MenuConfigListResponse, MenuConfigSummary, RouteNode, UpdateMenuConfigRequest } from "./types.js";
export declare class PrueRoutesApi {
    private readonly http;
    constructor(http: HttpCore);
    /** 拉取当前激活的动态路由树：GET /v1/pure/get-async-routes → RouteNode[]（无激活配置时为 []）。 */
    getAsync(): Promise<RouteNode[]>;
}
export declare class PrueMenuConfigsApi {
    private readonly http;
    constructor(http: HttpCore);
    /** 配置版本列表：GET /v1/pure/menu-configs → {items,total}。 */
    list(): Promise<MenuConfigListResponse>;
    /** 创建草稿版本：POST /v1/pure/menu-configs。 */
    create(request: CreateMenuConfigRequest): Promise<MenuConfigSummary>;
    /** 版本详情（含 routes_json）：GET /v1/pure/menu-configs/{id}。 */
    get(id: string): Promise<MenuConfigDetail>;
    /**
     * 更新草稿：PUT /v1/pure/menu-configs/{id}。缺 if_match_etag 返回 400；
     * ETag 冲突返回 409（error.body.data.current_etag 为服务端当前值）。
     */
    update(id: string, request: UpdateMenuConfigRequest): Promise<MenuConfigDetail>;
    /** 删除版本：DELETE /v1/pure/menu-configs/{id}（已发布版本返回 409）。 */
    remove(id: string): Promise<void>;
    /** 克隆为新草稿：POST /v1/pure/menu-configs/{id}/clone。 */
    clone(id: string): Promise<MenuConfigSummary>;
    /** 发布（draft → published 并激活）：POST /v1/pure/menu-configs/{id}/publish。 */
    publish(id: string): Promise<MenuConfigSummary>;
}
//# sourceMappingURL=configs.d.ts.map
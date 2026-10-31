import type { HttpCore } from "./http.js";
import type { ConfigItem, ConfigItemQuery, CreateConfigItem, PagedConfigItems, UpdateConfigItem } from "./types.js";
export declare class ConfigItemsApi {
    private readonly http;
    constructor(http: HttpCore);
    /** 分页查询配置项：GET /v1/config/item。 */
    page(query?: ConfigItemQuery): Promise<PagedConfigItems>;
    /** 查询单个配置项：GET /v1/config/item/{id}。 */
    get(id: string): Promise<ConfigItem>;
    /** 创建配置项：POST /v1/config/item。 */
    create(request: CreateConfigItem): Promise<ConfigItem>;
    /** 更新配置项：PUT /v1/config/item/{id}。 */
    update(id: string, request: UpdateConfigItem): Promise<ConfigItem>;
    /** 删除配置项：DELETE /v1/config/item/{id} → {success:true}。 */
    remove(id: string): Promise<{
        success: boolean;
    }>;
}
//# sourceMappingURL=items.d.ts.map
// 配置项管理（config-service，经网关 /v1/config/item*，需要鉴权）。

import type { HttpCore } from "./http.js";
import type {
  ConfigItem,
  ConfigItemQuery,
  CreateConfigItem,
  PagedConfigItems,
  UpdateConfigItem,
} from "./types.js";

export class ConfigItemsApi {
  constructor(private readonly http: HttpCore) {}

  /** 分页查询配置项：GET /v1/config/item。 */
  page(query: ConfigItemQuery = {}): Promise<PagedConfigItems> {
    return this.http.get<PagedConfigItems>("/v1/config/item", { params: query, auth: true });
  }

  /** 查询单个配置项：GET /v1/config/item/{id}。 */
  get(id: string): Promise<ConfigItem> {
    return this.http.get<ConfigItem>(`/v1/config/item/${encodeURIComponent(id)}`, { auth: true });
  }

  /** 创建配置项：POST /v1/config/item。 */
  create(request: CreateConfigItem): Promise<ConfigItem> {
    return this.http.post<ConfigItem>("/v1/config/item", request, { auth: true });
  }

  /** 更新配置项：PUT /v1/config/item/{id}。 */
  update(id: string, request: UpdateConfigItem): Promise<ConfigItem> {
    return this.http.put<ConfigItem>(`/v1/config/item/${encodeURIComponent(id)}`, request, { auth: true });
  }

  /** 删除配置项：DELETE /v1/config/item/{id} → {success:true}。 */
  remove(id: string): Promise<{ success: boolean }> {
    return this.http.delete<{ success: boolean }>(`/v1/config/item/${encodeURIComponent(id)}`, { auth: true });
  }
}

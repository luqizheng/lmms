// 动态路由与菜单配置版本（prue_backend，经网关 /v1/pure/**，需要鉴权——注意：
// 服务端自身不校验 JWT，鉴权完全依赖网关，这是上游已知的防护现状）。

import type { HttpCore } from "./http.js";
import { unwrap } from "./envelope.js";
import type {
  CreateMenuConfigRequest,
  MenuConfigDetail,
  MenuConfigListResponse,
  MenuConfigSummary,
  RouteNode,
  UpdateMenuConfigRequest,
} from "./types.js";

export class PrueRoutesApi {
  constructor(private readonly http: HttpCore) {}

  /** 拉取当前激活的动态路由树：GET /v1/pure/get-async-routes → RouteNode[]（无激活配置时为 []）。 */
  getAsync(): Promise<RouteNode[]> {
    return unwrap(this.http.get<{ code: number; message: string; data: RouteNode[] }>("/v1/pure/get-async-routes", { auth: true }));
  }
}

export class PrueMenuConfigsApi {
  constructor(private readonly http: HttpCore) {}

  /** 配置版本列表：GET /v1/pure/menu-configs → {items,total}。 */
  list(): Promise<MenuConfigListResponse> {
    return unwrap(this.http.get<{ code: number; message: string; data: MenuConfigListResponse }>("/v1/pure/menu-configs", { auth: true }));
  }

  /** 创建草稿版本：POST /v1/pure/menu-configs。 */
  create(request: CreateMenuConfigRequest): Promise<MenuConfigSummary> {
    return unwrap(this.http.post<{ code: number; message: string; data: MenuConfigSummary }>("/v1/pure/menu-configs", request, { auth: true }));
  }

  /** 版本详情（含 routes_json）：GET /v1/pure/menu-configs/{id}。 */
  get(id: string): Promise<MenuConfigDetail> {
    return unwrap(this.http.get<{ code: number; message: string; data: MenuConfigDetail }>(`/v1/pure/menu-configs/${encodeURIComponent(id)}`, { auth: true }));
  }

  /**
   * 更新草稿：PUT /v1/pure/menu-configs/{id}。缺 if_match_etag 返回 400；
   * ETag 冲突返回 409（error.body.data.current_etag 为服务端当前值）。
   */
  update(id: string, request: UpdateMenuConfigRequest): Promise<MenuConfigDetail> {
    return unwrap(this.http.put<{ code: number; message: string; data: MenuConfigDetail }>(`/v1/pure/menu-configs/${encodeURIComponent(id)}`, request, { auth: true }));
  }

  /** 删除版本：DELETE /v1/pure/menu-configs/{id}（已发布版本返回 409）。 */
  async remove(id: string): Promise<void> {
    await this.http.delete(`/v1/pure/menu-configs/${encodeURIComponent(id)}`, { auth: true });
  }

  /** 克隆为新草稿：POST /v1/pure/menu-configs/{id}/clone。 */
  clone(id: string): Promise<MenuConfigSummary> {
    return unwrap(this.http.post<{ code: number; message: string; data: MenuConfigSummary }>(`/v1/pure/menu-configs/${encodeURIComponent(id)}/clone`, undefined, { auth: true }));
  }

  /** 发布（draft → published 并激活）：POST /v1/pure/menu-configs/{id}/publish。 */
  publish(id: string): Promise<MenuConfigSummary> {
    return unwrap(this.http.post<{ code: number; message: string; data: MenuConfigSummary }>(`/v1/pure/menu-configs/${encodeURIComponent(id)}/publish`, undefined, { auth: true }));
  }
}

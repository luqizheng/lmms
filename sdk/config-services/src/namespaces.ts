// 命名空间管理（config-service，经网关 /v1/config/namespace*，需要鉴权）。

import type { HttpCore } from "./http.js";
import type {
  CreateNamespace,
  Namespace,
  NamespaceQuery,
  PagedNamespaces,
  UpdateNamespace,
} from "./types.js";

export class ConfigNamespacesApi {
  constructor(private readonly http: HttpCore) {}

  /** 分页查询命名空间：GET /v1/config/namespace。 */
  page(query: NamespaceQuery = {}): Promise<PagedNamespaces> {
    return this.http.get<PagedNamespaces>("/v1/config/namespace", { params: query, auth: true });
  }

  /** 查询单个命名空间：GET /v1/config/namespace/{id}。 */
  get(id: string): Promise<Namespace> {
    return this.http.get<Namespace>(`/v1/config/namespace/${encodeURIComponent(id)}`, { auth: true });
  }

  /** 创建命名空间：POST /v1/config/namespace。 */
  create(request: CreateNamespace): Promise<Namespace> {
    return this.http.post<Namespace>("/v1/config/namespace", request, { auth: true });
  }

  /** 更新命名空间：PUT /v1/config/namespace/{id}。 */
  update(id: string, request: UpdateNamespace): Promise<Namespace> {
    return this.http.put<Namespace>(`/v1/config/namespace/${encodeURIComponent(id)}`, request, { auth: true });
  }

  /** 删除命名空间：DELETE /v1/config/namespace/{id} → {success:true}。 */
  remove(id: string): Promise<{ success: boolean }> {
    return this.http.delete<{ success: boolean }>(`/v1/config/namespace/${encodeURIComponent(id)}`, { auth: true });
  }
}

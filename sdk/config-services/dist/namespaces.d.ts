import type { HttpCore } from "./http.js";
import type { CreateNamespace, Namespace, NamespaceQuery, PagedNamespaces, UpdateNamespace } from "./types.js";
export declare class ConfigNamespacesApi {
    private readonly http;
    constructor(http: HttpCore);
    /** 分页查询命名空间：GET /v1/config/namespace。 */
    page(query?: NamespaceQuery): Promise<PagedNamespaces>;
    /** 查询单个命名空间：GET /v1/config/namespace/{id}。 */
    get(id: string): Promise<Namespace>;
    /** 创建命名空间：POST /v1/config/namespace。 */
    create(request: CreateNamespace): Promise<Namespace>;
    /** 更新命名空间：PUT /v1/config/namespace/{id}。 */
    update(id: string, request: UpdateNamespace): Promise<Namespace>;
    /** 删除命名空间：DELETE /v1/config/namespace/{id} → {success:true}。 */
    remove(id: string): Promise<{
        success: boolean;
    }>;
}
//# sourceMappingURL=namespaces.d.ts.map
// 命名空间管理（config-service，经网关 /v1/config/namespace*，需要鉴权）。
export class ConfigNamespacesApi {
    http;
    constructor(http) {
        this.http = http;
    }
    /** 分页查询命名空间：GET /v1/config/namespace。 */
    page(query = {}) {
        return this.http.get("/v1/config/namespace", { params: query, auth: true });
    }
    /** 查询单个命名空间：GET /v1/config/namespace/{id}。 */
    get(id) {
        return this.http.get(`/v1/config/namespace/${encodeURIComponent(id)}`, { auth: true });
    }
    /** 创建命名空间：POST /v1/config/namespace。 */
    create(request) {
        return this.http.post("/v1/config/namespace", request, { auth: true });
    }
    /** 更新命名空间：PUT /v1/config/namespace/{id}。 */
    update(id, request) {
        return this.http.put(`/v1/config/namespace/${encodeURIComponent(id)}`, request, { auth: true });
    }
    /** 删除命名空间：DELETE /v1/config/namespace/{id} → {success:true}。 */
    remove(id) {
        return this.http.delete(`/v1/config/namespace/${encodeURIComponent(id)}`, { auth: true });
    }
}
//# sourceMappingURL=namespaces.js.map
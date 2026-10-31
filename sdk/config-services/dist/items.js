// 配置项管理（config-service，经网关 /v1/config/item*，需要鉴权）。
export class ConfigItemsApi {
    http;
    constructor(http) {
        this.http = http;
    }
    /** 分页查询配置项：GET /v1/config/item。 */
    page(query = {}) {
        return this.http.get("/v1/config/item", { params: query, auth: true });
    }
    /** 查询单个配置项：GET /v1/config/item/{id}。 */
    get(id) {
        return this.http.get(`/v1/config/item/${encodeURIComponent(id)}`, { auth: true });
    }
    /** 创建配置项：POST /v1/config/item。 */
    create(request) {
        return this.http.post("/v1/config/item", request, { auth: true });
    }
    /** 更新配置项：PUT /v1/config/item/{id}。 */
    update(id, request) {
        return this.http.put(`/v1/config/item/${encodeURIComponent(id)}`, request, { auth: true });
    }
    /** 删除配置项：DELETE /v1/config/item/{id} → {success:true}。 */
    remove(id) {
        return this.http.delete(`/v1/config/item/${encodeURIComponent(id)}`, { auth: true });
    }
}
//# sourceMappingURL=items.js.map
// 日志查询（audit-service，经网关 /v1/audit/login|operation，全部需要鉴权）。
// 路由事实源：services/audit-service/src/routes/。
export class AuditLogsApi {
    http;
    constructor(http) {
        this.http = http;
    }
    /** 分页查询登录日志：GET /v1/audit/login。 */
    queryLogin(query = {}) {
        return this.http.get("/v1/audit/login", { params: query, auth: true });
    }
    /** 分页查询操作日志：GET /v1/audit/operation。 */
    queryOperation(query = {}) {
        return this.http.get("/v1/audit/operation", { params: query, auth: true });
    }
}
//# sourceMappingURL=logs.js.map
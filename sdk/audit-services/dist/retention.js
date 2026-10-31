// 保留策略（audit-service，经网关 /v1/audit/retention/**，需要鉴权）。
export class AuditRetentionApi {
    http;
    constructor(http) {
        this.http = http;
    }
    /** 查询保留策略：GET /v1/audit/retention/policy。 */
    getPolicy() {
        return this.http.get("/v1/audit/retention/policy", { auth: true });
    }
    /** 更新保留策略：PUT /v1/audit/retention/policy。违反 hot<warm<cold 时返回 400（CFG_VALID_2003/2004/2005）。 */
    updatePolicy(policy) {
        return this.http.put("/v1/audit/retention/policy", policy, { auth: true });
    }
    /** 保留统计：GET /v1/audit/retention/stats。 */
    stats() {
        return this.http.get("/v1/audit/retention/stats", { auth: true });
    }
    /** 手动触发清理：POST /v1/audit/retention/cleanup。 */
    cleanup() {
        return this.http.post("/v1/audit/retention/cleanup", undefined, { auth: true });
    }
    /** 数据阶段判定：GET /v1/audit/retention/stage/{created_at}（RFC3339 路径参数）。 */
    dataStage(createdAt) {
        return this.http.get(`/v1/audit/retention/stage/${encodeURIComponent(createdAt)}`, {
            auth: true,
        });
    }
}
//# sourceMappingURL=retention.js.map
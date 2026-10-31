import type { HttpCore } from "./http.js";
import type { DataStageInfo, RetentionCleanupResult, RetentionPolicy, RetentionStats } from "./types.js";
export declare class AuditRetentionApi {
    private readonly http;
    constructor(http: HttpCore);
    /** 查询保留策略：GET /v1/audit/retention/policy。 */
    getPolicy(): Promise<RetentionPolicy>;
    /** 更新保留策略：PUT /v1/audit/retention/policy。违反 hot<warm<cold 时返回 400（CFG_VALID_2003/2004/2005）。 */
    updatePolicy(policy: RetentionPolicy): Promise<RetentionPolicy>;
    /** 保留统计：GET /v1/audit/retention/stats。 */
    stats(): Promise<RetentionStats>;
    /** 手动触发清理：POST /v1/audit/retention/cleanup。 */
    cleanup(): Promise<RetentionCleanupResult>;
    /** 数据阶段判定：GET /v1/audit/retention/stage/{created_at}（RFC3339 路径参数）。 */
    dataStage(createdAt: string): Promise<DataStageInfo>;
}
//# sourceMappingURL=retention.d.ts.map
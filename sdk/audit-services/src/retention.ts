// 保留策略（audit-service，经网关 /v1/audit/retention/**，需要鉴权）。

import type { HttpCore } from "./http.js";
import type {
  DataStageInfo,
  RetentionCleanupResult,
  RetentionPolicy,
  RetentionStats,
} from "./types.js";

export class AuditRetentionApi {
  constructor(private readonly http: HttpCore) {}

  /** 查询保留策略：GET /v1/audit/retention/policy。 */
  getPolicy(): Promise<RetentionPolicy> {
    return this.http.get<RetentionPolicy>("/v1/audit/retention/policy", { auth: true });
  }

  /** 更新保留策略：PUT /v1/audit/retention/policy。违反 hot<warm<cold 时返回 400（CFG_VALID_2003/2004/2005）。 */
  updatePolicy(policy: RetentionPolicy): Promise<RetentionPolicy> {
    return this.http.put<RetentionPolicy>("/v1/audit/retention/policy", policy, { auth: true });
  }

  /** 保留统计：GET /v1/audit/retention/stats。 */
  stats(): Promise<RetentionStats> {
    return this.http.get<RetentionStats>("/v1/audit/retention/stats", { auth: true });
  }

  /** 手动触发清理：POST /v1/audit/retention/cleanup。 */
  cleanup(): Promise<RetentionCleanupResult> {
    return this.http.post<RetentionCleanupResult>("/v1/audit/retention/cleanup", undefined, { auth: true });
  }

  /** 数据阶段判定：GET /v1/audit/retention/stage/{created_at}（RFC3339 路径参数）。 */
  dataStage(createdAt: string): Promise<DataStageInfo> {
    return this.http.get<DataStageInfo>(`/v1/audit/retention/stage/${encodeURIComponent(createdAt)}`, {
      auth: true,
    });
  }
}

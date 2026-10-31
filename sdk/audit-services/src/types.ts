// audit-service DTO 镜像。字段名以后端 serde 序列化为唯一事实源：
//   - services/audit-service/openapi.yaml（注意：retention cleanup/stage 的响应体
//     在 YAML 中标注为 {}，实际形状以 routes/retention.rs 处理器为准，已照实镜像）
// 成功响应为裸 JSON（无 {code,message,data} 信封）；错误体为 {code,message,request_id}。

// ---------- 日志 ----------

export interface LoginLog {
  id: number;
  username: string;
  success: boolean;
  ip_address: string;
  request_id: string;
  created_at: string;
  user_id: string | null;
  fail_reason: string | null;
  location: string | null;
  user_agent: string | null;
}

export interface OperationLog {
  id: number;
  user_id: string;
  action: string;
  resource: string;
  request_id: string;
  created_at: string;
  resource_id: string | null;
  before_value: string | null;
  after_value: string | null;
  ip_address: string | null;
  user_agent: string | null;
}

export interface PagedLoginLogs {
  items: LoginLog[];
  total: number;
  page: number;
  size: number;
}

export interface PagedOperationLogs {
  items: OperationLog[];
  total: number;
  page: number;
  size: number;
}

export interface LoginLogQuery {
  username?: string;
  success?: boolean;
  /** RFC3339，如 2026-10-31T00:00:00Z */
  start_time?: string;
  end_time?: string;
  page?: number;
  size?: number;
}

export interface OperationLogQuery {
  user_id?: string;
  action?: string;
  resource?: string;
  start_time?: string;
  end_time?: string;
  page?: number;
  size?: number;
}

// ---------- 保留策略 ----------

export interface RetentionPolicy {
  hot_days: number;
  warm_days: number;
  cold_days: number;
  archive_enabled: boolean;
  purge_enabled: boolean;
}

export interface RetentionStats {
  hot_count: number;
  warm_count: number;
  cold_count: number;
  expired_count: number;
  total_size_bytes: number;
  last_cleanup: string | null;
}

/** POST /v1/audit/retention/cleanup 的实际响应（openapi 未建模，取自 routes/retention.rs）。 */
export interface RetentionCleanupResult {
  deleted_count: number;
  cutoff_date: string;
  success: boolean;
}

export type DataStage = "Hot" | "Warm" | "Cold" | "Expired";

/** GET /v1/audit/retention/stage/{created_at} 的实际响应（openapi 未建模，取自处理器）。 */
export interface DataStageInfo {
  created_at: string;
  stage: DataStage | string;
  policy: {
    hot_days: number;
    warm_days: number;
    cold_days: number;
  };
}

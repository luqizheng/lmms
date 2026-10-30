// Auto-generated types from OpenAPI spec
// DO NOT EDIT

export type DataStage = "Hot" | "Warm" | "Cold" | "Expired";

export interface LoginLog {
  created_at: string;
  fail_reason?: string;
  id: number;
  ip_address: string;
  location?: string;
  request_id: string;
  success: boolean;
  user_agent?: string;
  user_id?: string;
  username: string;
}

export interface MigrationInfo {
  name: string;
  version: string;
}

export interface MigrationStateResponse {
  applied: string[];
  available: MigrationInfo[];
  current_version?: string;
  dialect: string;
  extra: string[];
  in_sync: boolean;
  missing: string[];
  service: string;
}

export interface OperationLog {
  action: string;
  after_value?: string;
  before_value?: string;
  created_at: string;
  id: number;
  ip_address?: string;
  request_id: string;
  resource: string;
  resource_id?: string;
  user_agent?: string;
  user_id: string;
}

export interface PagedLoginLogs {
  items: LoginLog[];
  page: number;
  size: number;
  total: number;
}

export interface PagedOperationLogs {
  items: OperationLog[];
  page: number;
  size: number;
  total: number;
}

export interface RetentionPolicy {
  archive_enabled: boolean;
  cold_days: number;
  hot_days: number;
  purge_enabled: boolean;
  warm_days: number;
}

export interface RetentionStats {
  cold_count: number;
  expired_count: number;
  hot_count: number;
  last_cleanup?: string;
  total_size_bytes: number;
  warm_count: number;
}

export interface ApiResponse<T = any> {
  code: string;
  message: string;
  data?: T;
  request_id?: string;
}

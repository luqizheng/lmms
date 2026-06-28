// Auto-generated types from OpenAPI spec
// DO NOT EDIT

export interface OperationLog {
  id: number;
  user_id: string;
  action: string;
  resource: string;
  resource_id?: string;
  before_value?: string;
  after_value?: string;
  request_id: string;
  ip_address?: string;
  user_agent?: string;
  created_at: string;
}

export interface PagedOperationLogs {
  items: OperationLog[];
  total: number;
  page: number;
  size: number;
}

export interface LoginLog {
  id: number;
  user_id: string;
  username: string;
  success: boolean;
  fail_reason?: string;
  ip_address?: string;
  location?: string;
  user_agent?: string;
  created_at: string;
}

export interface PagedLoginLogs {
  items: LoginLog[];
  total: number;
  page: number;
  size: number;
}

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
}

export interface ApiResponse<T = any> {
  code: string;
  message: string;
  data?: T;
  request_id?: string;
}

import { http } from "@/utils/http";

// ============== 操作日志 ==============
export interface OperationLog {
  id: number;
  user_id: string;
  action: string;
  resource: string;
  resource_id?: string;
  before_value?: string;
  after_value?: string;
  request_id?: string;
  ip_address?: string;
  user_agent?: string;
  created_at: string;
}

export interface OperationLogQuery {
  user_id?: string;
  action?: string;
  resource?: string;
  page?: number;
  size?: number;
}

export interface PagedOperationLogs {
  items: OperationLog[];
  total: number;
  page: number;
  size: number;
}

export function listOperationLogs(params?: OperationLogQuery) {
  return http.request<PagedOperationLogs>("get", "/audit/operation", {
    params
  });
}

// ============== 登录日志 ==============
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

export interface LoginLogQuery {
  page?: number;
  size?: number;
}

export interface PagedLoginLogs {
  items: LoginLog[];
  total: number;
  page: number;
  size: number;
}

export function listLoginLogs(params?: LoginLogQuery) {
  return http.request<PagedLoginLogs>("get", "/audit/login", { params });
}

// ============== 数据保留 ==============
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

export interface CleanupResult {
  deleted_count: number;
  success: boolean;
}

export function getRetentionStats() {
  return http.request<RetentionStats>("get", "/audit/retention/stats");
}

export function getRetentionPolicy() {
  return http.request<RetentionPolicy>("get", "/audit/retention/policy");
}

export function updateRetentionPolicy(data: RetentionPolicy) {
  return http.request<void>("put", "/audit/retention/policy", { data });
}

export function triggerRetentionCleanup() {
  return http.request<CleanupResult>("post", "/audit/retention/cleanup");
}

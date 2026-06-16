// Auto-generated API client from OpenAPI spec
// DO NOT EDIT

import type {
  OperationLog,
  CreateOperationLog,
  PagedOperationLogs,
  LoginLog as _LoginLog,
  CreateLoginLog,
  PagedLoginLogs,
  RetentionPolicy,
  RetentionStats,
  ApiResponse as _ApiResponse
} from "./audit.types";

export interface ClientConfig {
  baseUrl: string;
  headers?: Record<string, string>;
}

export class AuditClient {
  private baseUrl: string;
  private headers: Record<string, string>;

  constructor(config: ClientConfig) {
    this.baseUrl = config.baseUrl.replace(/\/$/, "");
    this.headers = {
      "Content-Type": "application/json",
      ...config.headers
    };
  }

  private async request<T>(
    method: string,
    path: string,
    params?: Record<string, any>,
    body?: any
  ): Promise<T> {
    const url = new URL(this.baseUrl + path, "http://localhost");

    if (params) {
      Object.entries(params).forEach(([key, value]) => {
        if (value !== undefined && value !== null) {
          url.searchParams.set(key, String(value));
        }
      });
    }

    const response = await fetch(url.toString(), {
      method,
      headers: this.headers,
      body: body ? JSON.stringify(body) : undefined
    });

    if (!response.ok) {
      throw new Error(`HTTP ${response.status}: ${response.statusText}`);
    }

    const text = await response.text();
    const contentType = response.headers.get("content-type") || "";

    if (contentType.includes("application/json")) {
      return JSON.parse(text);
    }
    return text as any;
  }

  // Operation Logs
  async createOperationLog(log: CreateOperationLog): Promise<{ id: number }> {
    return this.request("POST", "/audit/operation", undefined, log);
  }

  async queryOperationLogs(params?: {
    page?: number;
    size?: number;
    user_id?: string;
    action?: string;
    resource?: string;
  }): Promise<PagedOperationLogs> {
    return this.request("GET", "/audit/operation", params);
  }

  // Login Logs
  async createLoginLog(log: CreateLoginLog): Promise<void> {
    return this.request("POST", "/audit/login", undefined, log);
  }

  async queryLoginLogs(params?: {
    page?: number;
    size?: number;
  }): Promise<PagedLoginLogs> {
    return this.request("GET", "/audit/login", params);
  }

  // Export
  async exportOperationCsv(userId: string): Promise<string> {
    return this.request(
      "GET",
      `/audit/export/compliance/operation/csv/${userId}`
    );
  }

  async exportOperationJson(userId: string): Promise<OperationLog[]> {
    return this.request(
      "GET",
      `/audit/export/compliance/operation/json/${userId}`
    );
  }

  async exportLoginCsv(userId: string): Promise<string> {
    return this.request("GET", `/audit/export/compliance/login/csv/${userId}`);
  }

  // Retention
  async getRetentionStats(): Promise<RetentionStats> {
    return this.request("GET", "/audit/retention/stats");
  }

  async getRetentionPolicy(): Promise<RetentionPolicy> {
    return this.request("GET", "/audit/retention/policy");
  }

  async updateRetentionPolicy(
    policy: RetentionPolicy
  ): Promise<RetentionPolicy> {
    return this.request("PUT", "/audit/retention/policy", undefined, policy);
  }

  async triggerCleanup(): Promise<{ deleted_count: number; success: boolean }> {
    return this.request("POST", "/audit/retention/cleanup");
  }

  async getDataStage(createdAt: string): Promise<{
    created_at: string;
    stage: string;
    policy: RetentionPolicy;
  }> {
    return this.request(
      "GET",
      `/audit/retention/stage/${encodeURIComponent(createdAt)}`
    );
  }

  // Health
  async health(): Promise<{ status: string }> {
    return this.request("GET", "/health");
  }
}

export default AuditClient;

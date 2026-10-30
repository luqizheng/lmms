// Auto-generated API client from OpenAPI spec
// DO NOT EDIT

import type {
  MigrationStateResponse,
  PagedLoginLogs,
  PagedOperationLogs,
  RetentionPolicy,
  RetentionStats
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

  async exportLoginCsv(user_id: string): Promise<string> {
    return this.request("GET", `/audit/export/compliance/login/csv/${encodeURIComponent(user_id)}`, undefined, undefined);
  }

  async exportOperationCsv(user_id: string): Promise<string> {
    return this.request("GET", `/audit/export/compliance/operation/csv/${encodeURIComponent(user_id)}`, undefined, undefined);
  }

  async exportOperationJson(user_id: string): Promise<string> {
    return this.request("GET", `/audit/export/compliance/operation/json/${encodeURIComponent(user_id)}`, undefined, undefined);
  }

  async handler(): Promise<MigrationStateResponse> {
    return this.request("GET", "/audit/internal/migrations/state", undefined, undefined);
  }

  async queryLoginLogs(params?: { username?: string; success?: boolean; start_time?: string; end_time?: string; page?: number; size?: number }): Promise<PagedLoginLogs> {
    return this.request("GET", "/audit/login", params, undefined);
  }

  async queryOperationLogs(params?: { user_id?: string; action?: string; resource?: string; start_time?: string; end_time?: string; page?: number; size?: number }): Promise<PagedOperationLogs> {
    return this.request("GET", "/audit/operation", params, undefined);
  }

  async triggerCleanup(): Promise<any> {
    return this.request("POST", "/audit/retention/cleanup", undefined, undefined);
  }

  async getRetentionPolicy(): Promise<RetentionPolicy> {
    return this.request("GET", "/audit/retention/policy", undefined, undefined);
  }

  async updateRetentionPolicy(body: RetentionPolicy): Promise<RetentionPolicy> {
    return this.request("PUT", "/audit/retention/policy", undefined, body);
  }

  async getDataStage(created_at: string): Promise<any> {
    return this.request("GET", `/audit/retention/stage/${encodeURIComponent(created_at)}`, undefined, undefined);
  }

  async getRetentionStats(): Promise<RetentionStats> {
    return this.request("GET", "/audit/retention/stats", undefined, undefined);
  }

  async health(): Promise<{ status: string }> {
    return this.request("GET", "/health");
  }
}

export default AuditClient;

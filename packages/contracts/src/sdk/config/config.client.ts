// Auto-generated API client from OpenAPI spec
// DO NOT EDIT

import type {
  ConfigItem,
  CreateConfigItem,
  CreateNamespace,
  MigrationStateResponse,
  Namespace,
  PagedConfigItems,
  PagedNamespaces,
  UpdateConfigItem,
  UpdateNamespace
} from "./config.types";

export interface ClientConfig {
  baseUrl: string;
  headers?: Record<string, string>;
}

export class ConfigClient {
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

  async handler(): Promise<MigrationStateResponse> {
    return this.request("GET", "/cfg/internal/migrations/state", undefined, undefined);
  }

  async exportConfig(namespace_id: string): Promise<string> {
    return this.request("GET", `/config/export/${encodeURIComponent(namespace_id)}`, undefined, undefined);
  }

  async importConfig(namespace_id: string): Promise<any> {
    return this.request("POST", `/config/import/${encodeURIComponent(namespace_id)}`, undefined, undefined);
  }

  async queryConfigItems(params?: { namespace_id?: string; key?: string; page?: number; size?: number }): Promise<PagedConfigItems> {
    return this.request("GET", "/config/item", params, undefined);
  }

  async createConfigItem(body: CreateConfigItem): Promise<ConfigItem> {
    return this.request("POST", "/config/item", undefined, body);
  }

  async getConfigItem(id: string): Promise<ConfigItem> {
    return this.request("GET", `/config/item/${encodeURIComponent(id)}`, undefined, undefined);
  }

  async updateConfigItem(id: string, body: UpdateConfigItem): Promise<ConfigItem> {
    return this.request("PUT", `/config/item/${encodeURIComponent(id)}`, undefined, body);
  }

  async deleteConfigItem(id: string): Promise<any> {
    return this.request("DELETE", `/config/item/${encodeURIComponent(id)}`, undefined, undefined);
  }

  async queryNamespaces(params?: { app?: string; env?: string; page?: number; size?: number }): Promise<PagedNamespaces> {
    return this.request("GET", "/config/namespace", params, undefined);
  }

  async createNamespace(body: CreateNamespace): Promise<Namespace> {
    return this.request("POST", "/config/namespace", undefined, body);
  }

  async getNamespace(id: string): Promise<Namespace> {
    return this.request("GET", `/config/namespace/${encodeURIComponent(id)}`, undefined, undefined);
  }

  async updateNamespace(id: string, body: UpdateNamespace): Promise<Namespace> {
    return this.request("PUT", `/config/namespace/${encodeURIComponent(id)}`, undefined, body);
  }

  async deleteNamespace(id: string): Promise<any> {
    return this.request("DELETE", `/config/namespace/${encodeURIComponent(id)}`, undefined, undefined);
  }

  async watchConfig(params?: { namespace_id?: string; timeout?: number }): Promise<any> {
    return this.request("GET", "/config/watch", params, undefined);
  }

  async health(): Promise<{ status: string }> {
    return this.request("GET", "/health");
  }
}

export default ConfigClient;

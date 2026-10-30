// Auto-generated API client from OpenAPI spec
// DO NOT EDIT

import type {
  CreatePresignUrl,
  FileInfo,
  MigrationStateResponse,
  PagedFiles,
  PresignUrlResponse
} from "./file.types";

export interface ClientConfig {
  baseUrl: string;
  headers?: Record<string, string>;
}

export class FileClient {
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
    return this.request("GET", "/file/internal/migrations/state", undefined, undefined);
  }

  async queryFiles(params?: { tenant_id?: string; user_id?: string; filename?: string; page?: number; size?: number }): Promise<PagedFiles> {
    return this.request("GET", "/file/list", params, undefined);
  }

  async createPresign(body: CreatePresignUrl): Promise<PresignUrlResponse> {
    return this.request("POST", "/file/presign", undefined, body);
  }

  async uploadFile(): Promise<FileInfo> {
    return this.request("POST", "/file/upload", undefined, undefined);
  }

  async downloadFile(id: string): Promise<string> {
    return this.request("GET", `/file/${encodeURIComponent(id)}`, undefined, undefined);
  }

  async deleteFile(id: string): Promise<any> {
    return this.request("DELETE", `/file/${encodeURIComponent(id)}`, undefined, undefined);
  }

  async getFileInfo(id: string): Promise<FileInfo> {
    return this.request("GET", `/file/${encodeURIComponent(id)}/info`, undefined, undefined);
  }

  async health(): Promise<{ status: string }> {
    return this.request("GET", "/health");
  }
}

export default FileClient;

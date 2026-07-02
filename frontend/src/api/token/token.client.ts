// Auto-generated API client from OpenAPI spec
// DO NOT EDIT

import type {
  AccessRequest,
  AccessResponse,
  ApiKeyCreated,
  ClientDto,
  CreateApiKeyRequest,
  CreateRequest,
  CreateResponse,
  ListApiKeysResponse,
  ListResponse,
  RefreshRequest,
  RefreshResponse,
  ResetSecretResponse,
  RevokeRequest,
  RevokeResponse,
  UpdateRequest,
  UserJwtRequest,
  UserJwtResponse
} from "./token.types";

export interface ClientConfig {
  baseUrl: string;
  headers?: Record<string, string>;
}

export class TokenClient {
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

  async handleCreate(body: CreateRequest): Promise<CreateResponse> {
    return this.request("POST", "/admin/oauth-clients", undefined, body);
  }

  async handleUpdate(
    client_id: string,
    body: UpdateRequest
  ): Promise<ClientDto> {
    return this.request(
      "PUT",
      `/admin/oauth-clients/${encodeURIComponent(client_id)}`,
      undefined,
      body
    );
  }

  async handleResetSecret(client_id: string): Promise<ResetSecretResponse> {
    return this.request(
      "POST",
      `/admin/oauth-clients/${encodeURIComponent(client_id)}/reset-secret`,
      undefined,
      undefined
    );
  }

  async handleListApiKeys(params?: {
    keyword?: string;
    page?: number;
    size?: number;
  }): Promise<ListApiKeysResponse> {
    return this.request("GET", "/api-keys", params, undefined);
  }

  async handleCreateApiKey(body: CreateApiKeyRequest): Promise<ApiKeyCreated> {
    return this.request("POST", "/api-keys", undefined, body);
  }

  async handleList(params?: {
    keyword?: string;
    status?: string;
    page?: number;
    size?: number;
  }): Promise<ListResponse> {
    return this.request(
      "GET",
      "/api/v1/admin/oauth-clients",
      params,
      undefined
    );
  }

  async handleDelete(client_id: string): Promise<void> {
    return this.request(
      "DELETE",
      `/api/v1/admin/oauth-clients/${encodeURIComponent(client_id)}`,
      undefined,
      undefined
    );
  }

  async handleAccess(body: AccessRequest): Promise<AccessResponse> {
    return this.request("POST", "/tokens/access", undefined, body);
  }

  async handleRefresh(body: RefreshRequest): Promise<RefreshResponse> {
    return this.request("POST", "/tokens/refresh", undefined, body);
  }

  async handleRevoke(body: RevokeRequest): Promise<RevokeResponse> {
    return this.request("POST", "/tokens/revoke", undefined, body);
  }

  async handleUserJwt(body: UserJwtRequest): Promise<UserJwtResponse> {
    return this.request("POST", "/tokens/user-jwt", undefined, body);
  }

  async health(): Promise<{ status: string }> {
    return this.request("GET", "/health");
  }
}

export default TokenClient;

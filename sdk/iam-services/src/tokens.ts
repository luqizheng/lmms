// token 管理（token-service，经网关 /v1/tokens/*、/v1/api-keys）。
// 路由事实源：services/token-service/src/main.rs、infrastructure/config/api-gateway/routes/token.router.toml。
// 说明：这是 OAuth 客户端链路（client_credentials / authorization_code / 带轮换的 refresh），
// 与 iam 登录链路（AuthApi.login/refresh）相互独立。

import type { HttpCore } from "./http.js";
import type {
  ApiKeyCreated,
  ApiKeyListResponse,
  CreateApiKeyRequest,
  CreateServiceJwtGrantRequest,
  Jwks,
  ServiceJwtGrant,
  ServiceJwtGrantListQuery,
  ServiceJwtGrantListResponse,
  SetServiceJwtGrantStatusRequest,
  TokenAccessRequest,
  TokenRefreshRequest,
  TokenResponse,
  TokenRevokeRequest,
  TokenRevokeResponse,
} from "./types.js";

export class ApiKeysApi {
  constructor(private readonly http: HttpCore) {}

  /** 列出 API Key / OAuth 客户端：GET /v1/api-keys（需鉴权）。后端返回 {items,total,page,size}。 */
  list(
    query: { keyword?: string; page?: number; size?: number } = {}
  ): Promise<ApiKeyListResponse> {
    return this.http.get<ApiKeyListResponse>("/v1/api-keys", {
      params: query,
      auth: true,
    });
  }

  /** 创建 API Key：POST /v1/api-keys（需鉴权）。api_key_secret 仅此一次返回。 */
  create(request: CreateApiKeyRequest): Promise<ApiKeyCreated> {
    return this.http.post<ApiKeyCreated>("/v1/api-keys", request, { auth: true });
  }
}

export class TokensApi {
  /** API Key / OAuth 客户端管理。 */
  readonly apiKeys: ApiKeysApi;

  /** Service JWT audience grant 管理。 */
  readonly serviceJwtGrants: ServiceJwtGrantsApi;

  constructor(private readonly http: HttpCore) {
    this.apiKeys = new ApiKeysApi(http);
    this.serviceJwtGrants = new ServiceJwtGrantsApi(http);
  }

  /** 签发 access token：POST /v1/tokens/access（公开）。 */
  issueAccess(request: TokenAccessRequest): Promise<TokenResponse> {
    return this.http.post<TokenResponse>("/v1/tokens/access", request);
  }

  /** OAuth 刷新（单次使用轮换 + 重放检测）：POST /v1/tokens/refresh（公开）。 */
  refresh(request: TokenRefreshRequest): Promise<TokenResponse> {
    return this.http.post<TokenResponse>("/v1/tokens/refresh", request);
  }

  /** 按 jti 拉黑吊销 access token：POST /v1/tokens/revoke。 */
  revoke(request: TokenRevokeRequest): Promise<TokenRevokeResponse> {
    return this.http.post<TokenRevokeResponse>("/v1/tokens/revoke", request);
  }

  /** 验签公钥集：GET /v1/tokens/.well-known/jwks.json（公开，RS256）。 */
  jwks(): Promise<Jwks> {
    return this.http.get<Jwks>("/v1/tokens/.well-known/jwks.json");
  }
}

/**
 * Service JWT audience grant 管理（token-service web 管理入口）。
 *
 * 路径 `/v1/admin/service-jwt-grants*`（经网关转发到 token-service）。
 * 鉴权：登录态 access token + `TOKEN_SERVICE_JWT_GRANT_*` 权限码在服务端强制
 * 校验；无权限返回 403，重复创建 / 保护行返回 409。
 */
export class ServiceJwtGrantsApi {
  constructor(private readonly http: HttpCore) {}

  /** 分页查询 grant：GET /v1/admin/service-jwt-grants。 */
  list(query: ServiceJwtGrantListQuery = {}): Promise<ServiceJwtGrantListResponse> {
    return this.http.get<ServiceJwtGrantListResponse>("/v1/admin/service-jwt-grants", {
      params: query,
      auth: true,
    });
  }

  /** 查单条 grant：GET /v1/admin/service-jwt-grants/{owner}/{audience}。 */
  get(owner: string, audience: string): Promise<ServiceJwtGrant> {
    return this.http.get<ServiceJwtGrant>(
      `/v1/admin/service-jwt-grants/${encodeURIComponent(owner)}/${encodeURIComponent(audience)}`,
      { auth: true },
    );
  }

  /** 创建 grant：POST /v1/admin/service-jwt-grants（重复 → 409）。 */
  create(request: CreateServiceJwtGrantRequest): Promise<ServiceJwtGrant> {
    return this.http.post<ServiceJwtGrant>("/v1/admin/service-jwt-grants", request, {
      auth: true,
    });
  }

  /** 启用 / 禁用：PUT /v1/admin/service-jwt-grants/{owner}/{audience}/status。 */
  setStatus(
    owner: string,
    audience: string,
    request: SetServiceJwtGrantStatusRequest,
  ): Promise<ServiceJwtGrant> {
    return this.http.put<ServiceJwtGrant>(
      `/v1/admin/service-jwt-grants/${encodeURIComponent(owner)}/${encodeURIComponent(audience)}/status`,
      request,
      { auth: true },
    );
  }

  /** 删除 grant：DELETE /v1/admin/service-jwt-grants/{owner}/{audience}（204）。 */
  async remove(owner: string, audience: string): Promise<void> {
    await this.http.delete(
      `/v1/admin/service-jwt-grants/${encodeURIComponent(owner)}/${encodeURIComponent(audience)}`,
      { auth: true },
    );
  }
}

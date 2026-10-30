// token 管理（token-service，经网关 /v1/tokens/*、/v1/api-keys）。
// 路由事实源：services/token-service/src/main.rs、infrastructure/config/api-gateway/routes/token.router.toml。
// 说明：这是 OAuth 客户端链路（client_credentials / authorization_code / 带轮换的 refresh），
// 与 iam 登录链路（AuthApi.login/refresh）相互独立。

import type { HttpCore } from "./http.js";
import type {
  ApiKeyCreated,
  ApiKeyListResponse,
  CreateApiKeyRequest,
  Jwks,
  TokenAccessRequest,
  TokenRefreshRequest,
  TokenResponse,
  TokenRevokeRequest,
  TokenRevokeResponse,
} from "./types.js";

export class ApiKeysApi {
  constructor(private readonly http: HttpCore) {}

  /** 列出 API Key / OAuth 客户端：GET /v1/api-keys（需鉴权）。 */
  list(): Promise<ApiKeyListResponse> {
    return this.http.get<ApiKeyListResponse>("/v1/api-keys", { auth: true });
  }

  /** 创建 API Key：POST /v1/api-keys（需鉴权）。api_key_secret 仅此一次返回。 */
  create(request: CreateApiKeyRequest): Promise<ApiKeyCreated> {
    return this.http.post<ApiKeyCreated>("/v1/api-keys", request, { auth: true });
  }
}

export class TokensApi {
  /** API Key / OAuth 客户端管理。 */
  readonly apiKeys: ApiKeysApi;

  constructor(private readonly http: HttpCore) {
    this.apiKeys = new ApiKeysApi(http);
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

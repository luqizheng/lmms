package io.digua.token;

import io.digua.common.SdkClient;
import io.digua.common.SdkConfig;
import io.digua.auth.ClientCredentialsProvider;
import io.digua.auth.CredentialProvider;
import io.digua.http.HttpRequest;
import io.digua.http.HttpResponse;
import io.digua.model.ResourceTokenInfo;
import io.digua.model.TokenInfo;

import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.concurrent.CompletableFuture;

/**
 * client → token-service SDK。
 * 用 client_credentials 流程签发、刷新、撤销令牌，并管理 api_key。
 */
public class TokenServerClientClient extends SdkClient {
    private final String basePath;

    private TokenServerClientClient(SdkConfig config, CredentialProvider provider, String basePath) {
        super(config, provider);
        this.basePath = basePath;
    }

    public static TokenServerClientClient create(String baseUrl, String apiKey, String secret, String code) {
        SdkConfig cfg = new SdkConfig(baseUrl);
        CredentialProvider p = new ClientCredentialsProvider(cfg, apiKey, secret, code);
        return new TokenServerClientClient(cfg, p, "/v1");
    }

    public static TokenServerClientClient create(SdkConfig config, String apiKey, String secret, String code) {
        CredentialProvider p = new ClientCredentialsProvider(config, apiKey, secret, code);
        return new TokenServerClientClient(config, p, "/v1");
    }

    private String url(String suffix) {
        return config.getBaseUrl() + basePath + suffix;
    }

    public TokenInfo refresh(String refreshToken, String clientId, String clientSecret) {
        Map<String, Object> payload = new LinkedHashMap<>();
        payload.put("refresh_token", refreshToken);
        payload.put("client_id", clientId);
        payload.put("client_secret", clientSecret);
        HttpResponse r = executeWithAuth(HttpRequest.post(url("/tokens/refresh")).jsonBody(payload));
        return mapBody(r, TokenInfo.class);
    }

    public CompletableFuture<TokenInfo> refreshAsync(String refreshToken, String clientId, String clientSecret) {
        return wrapAsync(() -> refresh(refreshToken, clientId, clientSecret));
    }

    public void revoke(String tokenJti, long expiresIn) {
        Map<String, Object> payload = new LinkedHashMap<>();
        payload.put("token_jti", tokenJti);
        payload.put("expires_in", expiresIn);
        HttpResponse r = executeWithAuth(HttpRequest.post(url("/tokens/revoke")).jsonBody(payload));
        ensureSuccess(r);
    }

    public CompletableFuture<Void> revokeAsync(String tokenJti, long expiresIn) {
        return wrapAsync(() -> { revoke(tokenJti, expiresIn); return null; });
    }

    public ResourceTokenInfo issueResource(String resourceType, String resourceId, String permission, Long ttlSec) {
        Map<String, Object> payload = new LinkedHashMap<>();
        payload.put("resource_type", resourceType);
        payload.put("resource_id", resourceId);
        payload.put("permission", permission);
        if (ttlSec != null) {
            payload.put("ttl_sec", ttlSec);
        }
        HttpResponse r = executeWithAuth(HttpRequest.post(url("/tokens/resource")).jsonBody(payload));
        return mapBody(r, ResourceTokenInfo.class);
    }

    public ApiKeyInfo createApiKey(String name, List<String> permissions, List<String> grantTypes) {
        Map<String, Object> payload = new LinkedHashMap<>();
        payload.put("name", name);
        payload.put("permissions", permissions);
        payload.put("grant_types", grantTypes);
        HttpResponse r = executeWithAuth(HttpRequest.post(url("/api-keys")).jsonBody(payload));
        return mapBody(r, ApiKeyInfo.class);
    }

    public String jwks() {
        HttpResponse r = executeWithAuth(HttpRequest.get(url("/tokens/.well-known/jwks.json")));
        return r.getBody();
    }
}

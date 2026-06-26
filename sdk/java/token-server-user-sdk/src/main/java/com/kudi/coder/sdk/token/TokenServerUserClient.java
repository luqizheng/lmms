package com.kudi.coder.sdk.token;

import com.kudi.coder.sdk.common.SdkClient;
import com.kudi.coder.sdk.common.SdkConfig;
import com.kudi.coder.sdk.common.auth.CredentialProvider;
import com.kudi.coder.sdk.common.auth.PasswordCredentialProvider;
import com.kudi.coder.sdk.common.http.HttpRequest;
import com.kudi.coder.sdk.common.http.HttpResponse;
import com.kudi.coder.sdk.common.model.ResourceTokenInfo;
import com.kudi.coder.sdk.common.model.TokenInfo;

import java.util.LinkedHashMap;
import java.util.Map;
import java.util.concurrent.CompletableFuture;

/**
 * user → token-service SDK。
 * 终端用户先经 iam-service 登录拿 JWT，再用同一 JWT 调用 token-service。
 */
public class TokenServerUserClient extends SdkClient {
    private final String basePath;

    private TokenServerUserClient(SdkConfig config, CredentialProvider provider, String basePath) {
        super(config, provider);
        this.basePath = basePath;
    }

    public static TokenServerUserClient login(String baseUrl, String username, String password) {
        SdkConfig cfg = new SdkConfig(baseUrl);
        CredentialProvider p = new PasswordCredentialProvider(cfg, username, password);
        return new TokenServerUserClient(cfg, p, "/v1");
    }

    public static TokenServerUserClient login(SdkConfig config, String username, String password) {
        CredentialProvider p = new PasswordCredentialProvider(config, username, password);
        return new TokenServerUserClient(config, p, "/v1");
    }

    private String url(String suffix) {
        return config.getBaseUrl() + basePath + suffix;
    }

    public TokenInfo exchangeCode(String code, String clientId, String clientSecret) {
        Map<String, Object> payload = new LinkedHashMap<>();
        payload.put("grant_type", "authorization_code");
        payload.put("code", code);
        payload.put("client_id", clientId);
        payload.put("client_secret", clientSecret);
        HttpResponse r = executeWithAuth(HttpRequest.post(url("/tokens/access")).jsonBody(payload));
        return mapBody(r, TokenInfo.class);
    }

    public CompletableFuture<TokenInfo> exchangeCodeAsync(String code, String clientId, String clientSecret) {
        return wrapAsync(() -> exchangeCode(code, clientId, clientSecret));
    }

    public TokenInfo refresh(String refreshToken, String clientId, String clientSecret) {
        Map<String, Object> payload = new LinkedHashMap<>();
        payload.put("refresh_token", refreshToken);
        payload.put("client_id", clientId);
        payload.put("client_secret", clientSecret);
        HttpResponse r = executeWithAuth(HttpRequest.post(url("/tokens/refresh")).jsonBody(payload));
        return mapBody(r, TokenInfo.class);
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

    public String jwks() {
        HttpResponse r = executeWithAuth(HttpRequest.get(url("/tokens/.well-known/jwks.json")));
        return r.getBody();
    }
}

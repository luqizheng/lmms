package com.kudi.coder.sdk.token;

import com.kudi.coder.sdk.common.SdkClient;
import com.kudi.coder.sdk.common.SdkConfig;
import com.kudi.coder.sdk.common.auth.ClientCredentialsProvider;
import com.kudi.coder.sdk.common.auth.CredentialProvider;
import com.kudi.coder.sdk.common.http.HttpRequest;
import com.kudi.coder.sdk.common.http.HttpResponse;
import com.kudi.coder.sdk.common.model.TokenInfo;

import java.util.HashMap;
import java.util.Map;
import java.util.concurrent.CompletableFuture;

/**
 * client → token-server SDK。
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
        return new TokenServerClientClient(cfg, p, "/api");
    }

    public static TokenServerClientClient create(SdkConfig config, String apiKey, String secret, String code) {
        CredentialProvider p = new ClientCredentialsProvider(config, apiKey, secret, code);
        return new TokenServerClientClient(config, p, "/api");
    }

    private String url(String suffix) {
        return config.getBaseUrl() + basePath + suffix;
    }

    public TokenInfo refresh(String refreshToken, String clientId, String clientSecret) {
        Map<String, String> form = new HashMap<>();
        form.put("grant_type", "refresh_token");
        form.put("refresh_token", refreshToken);
        form.put("client_id", clientId);
        form.put("client_secret", clientSecret);
        HttpResponse r = executeWithAuth(HttpRequest.post(url("/tokens/refresh"))
                .body(formBody(form), "application/x-www-form-urlencoded"));
        return mapBody(r, TokenInfo.class);
    }

    public CompletableFuture<TokenInfo> refreshAsync(String refreshToken, String clientId, String clientSecret) {
        return wrapAsync(() -> refresh(refreshToken, clientId, clientSecret));
    }

    public void revoke(String token, String clientId, String clientSecret) {
        Map<String, String> form = new HashMap<>();
        form.put("token", token);
        form.put("client_id", clientId);
        form.put("client_secret", clientSecret);
        HttpResponse r = executeWithAuth(HttpRequest.post(url("/tokens/revoke"))
                .body(formBody(form), "application/x-www-form-urlencoded"));
        ensureSuccess(r);
    }

    public TokenInfo issueResource(String userId, String resource, String scope,
                                   String clientId, String clientSecret) {
        Map<String, String> form = new HashMap<>();
        form.put("client_id", clientId);
        form.put("client_secret", clientSecret);
        form.put("user_id", userId);
        form.put("resource", resource);
        form.put("scope", scope);
        HttpResponse r = executeWithAuth(HttpRequest.post(url("/tokens/resource"))
                .body(formBody(form), "application/x-www-form-urlencoded"));
        return mapBody(r, TokenInfo.class);
    }

    public ApiKeyInfo createApiKey(String name, String clientId, String clientSecret) {
        Map<String, String> form = new HashMap<>();
        form.put("client_id", clientId);
        form.put("client_secret", clientSecret);
        form.put("name", name);
        HttpResponse r = executeWithAuth(HttpRequest.post(url("/api-keys"))
                .body(formBody(form), "application/x-www-form-urlencoded"));
        return mapBody(r, ApiKeyInfo.class);
    }

    public String jwks() {
        HttpResponse r = executeWithAuth(HttpRequest.get(url("/tokens/.well-known/jwks.json")));
        return r.getBody();
    }

    private String formBody(Map<String, String> form) {
        StringBuilder sb = new StringBuilder();
        for (Map.Entry<String, String> e : form.entrySet()) {
            if (sb.length() > 0) sb.append('&');
            try {
                sb.append(e.getKey()).append('=')
                        .append(java.net.URLEncoder.encode(e.getValue(), "UTF-8"));
            } catch (java.io.UnsupportedEncodingException ex) {
                throw new com.kudi.coder.sdk.common.exception.SdkException("URL encoding failed", ex);
            }
        }
        return sb.toString();
    }
}

package com.kudi.coder.sdk.token;

import com.fasterxml.jackson.annotation.JsonIgnoreProperties;
import com.fasterxml.jackson.annotation.JsonProperty;
import com.kudi.coder.sdk.common.SdkClient;
import com.kudi.coder.sdk.common.SdkConfig;
import com.kudi.coder.sdk.common.auth.CredentialProvider;
import com.kudi.coder.sdk.common.auth.PasswordCredentialProvider;
import com.kudi.coder.sdk.common.http.HttpRequest;
import com.kudi.coder.sdk.common.http.HttpResponse;
import com.kudi.coder.sdk.common.model.TokenInfo;

import java.util.HashMap;
import java.util.Map;
import java.util.concurrent.CompletableFuture;

/**
 * user → token-server SDK。
 * 主要给终端用户场景提供登录/刷新/登出/资源令牌。
 */
public class TokenServerUserClient extends SdkClient {
    private final String basePath;

    private TokenServerUserClient(SdkConfig config, CredentialProvider provider, String basePath) {
        super(config, provider);
        this.basePath = basePath;
    }

    public static TokenServerUserClient login(String baseUrl, String username, String password) {
        SdkConfig cfg = new SdkConfig(baseUrl);
        CredentialProvider p = new PasswordCredentialProvider(cfg, username, password,
                "/api/tokens/access");
        return new TokenServerUserClient(cfg, p, "/api");
    }

    public static TokenServerUserClient login(SdkConfig config, String username, String password) {
        CredentialProvider p = new PasswordCredentialProvider(config, username, password,
                "/api/tokens/access");
        return new TokenServerUserClient(config, p, "/api");
    }

    private String url(String suffix) {
        return config.getBaseUrl() + basePath + suffix;
    }

    public TokenInfo exchangeCode(String code, String clientId, String clientSecret) {
        Map<String, String> form = new HashMap<>();
        form.put("grant_type", "authorization_code");
        form.put("code", code);
        form.put("client_id", clientId);
        form.put("client_secret", clientSecret);
        HttpResponse r = executeWithAuth(HttpRequest.post(url("/tokens/access"))
                .body(formBody(form), "application/x-www-form-urlencoded"));
        return mapBody(r, TokenInfo.class);
    }

    public CompletableFuture<TokenInfo> exchangeCodeAsync(String code, String clientId, String clientSecret) {
        return wrapAsync(() -> exchangeCode(code, clientId, clientSecret));
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

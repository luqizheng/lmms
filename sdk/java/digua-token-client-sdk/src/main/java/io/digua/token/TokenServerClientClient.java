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

    // ── Service JWT audience grants（WP-LMMS-SERVICE-JWT-AUDIENCE-ADMIN-01）──
    //
    // 走网关路径 /v1/admin/service-jwt-grants*（token-service 的 web 管理入口，
    // user-jwt / 带 permissions 的 access token + TOKEN_SERVICE_JWT_GRANT_*
    // 权限码鉴权）。重复创建 → 409，system_managed 行改删 → 409，均由
    // ensureSuccess 抛出（带 HTTP 状态）。

    /** 分页查询 grant。参数可传 {@code null} 表示不过滤；page/size 缺省由服务端决定。 */
    public ServiceJwtGrantPage listServiceJwtGrants(
            String owner, String audience, String status, Integer page, Integer size) {
        StringBuilder qs = new StringBuilder();
        appendParam(qs, "owner", owner);
        appendParam(qs, "audience", audience);
        appendParam(qs, "status", status);
        if (page != null) {
            appendParam(qs, "page", String.valueOf(page));
        }
        if (size != null) {
            appendParam(qs, "size", String.valueOf(size));
        }
        String suffix = "/admin/service-jwt-grants" + qs;
        HttpResponse r = executeWithAuth(HttpRequest.get(url(suffix)));
        return mapBody(r, ServiceJwtGrantPage.class);
    }

    /** 查单条 grant（不存在 → 404 → ensureSuccess 抛出）。 */
    public ServiceJwtGrantInfo getServiceJwtGrant(String owner, String audience) {
        HttpResponse r = executeWithAuth(
                HttpRequest.get(url("/admin/service-jwt-grants/" + owner + "/" + audience)));
        return mapBody(r, ServiceJwtGrantInfo.class);
    }

    /** 创建 grant（owner/audience 须匹配 [a-zA-Z0-9_-]，1..=64；重复 → 409）。 */
    public ServiceJwtGrantInfo createServiceJwtGrant(String owner, String audience) {
        Map<String, Object> payload = new LinkedHashMap<>();
        payload.put("owner", owner);
        payload.put("audience", audience);
        HttpResponse r = executeWithAuth(
                HttpRequest.post(url("/admin/service-jwt-grants")).jsonBody(payload));
        return mapBody(r, ServiceJwtGrantInfo.class);
    }

    /** 启用 / 禁用 grant（{@code status}: "active" | "disabled"；非法 → 400）。 */
    public ServiceJwtGrantInfo setServiceJwtGrantStatus(String owner, String audience, String status) {
        Map<String, Object> payload = new LinkedHashMap<>();
        payload.put("status", status);
        HttpResponse r = executeWithAuth(HttpRequest
                .put(url("/admin/service-jwt-grants/" + owner + "/" + audience + "/status"))
                .jsonBody(payload));
        return mapBody(r, ServiceJwtGrantInfo.class);
    }

    /** 删除 grant（不存在/已删 → 404；system_managed → 409）。 */
    public void deleteServiceJwtGrant(String owner, String audience) {
        HttpResponse r = executeWithAuth(
                HttpRequest.delete(url("/admin/service-jwt-grants/" + owner + "/" + audience)));
        ensureSuccess(r);
    }

    public CompletableFuture<ServiceJwtGrantPage> listServiceJwtGrantsAsync(
            String owner, String audience, String status, Integer page, Integer size) {
        return wrapAsync(() -> listServiceJwtGrants(owner, audience, status, page, size));
    }

    public CompletableFuture<ServiceJwtGrantInfo> createServiceJwtGrantAsync(String owner, String audience) {
        return wrapAsync(() -> createServiceJwtGrant(owner, audience));
    }

    public CompletableFuture<ServiceJwtGrantInfo> setServiceJwtGrantStatusAsync(
            String owner, String audience, String status) {
        return wrapAsync(() -> setServiceJwtGrantStatus(owner, audience, status));
    }

    public CompletableFuture<Void> deleteServiceJwtGrantAsync(String owner, String audience) {
        return wrapAsync(() -> {
            deleteServiceJwtGrant(owner, audience);
            return null;
        });
    }

    private static void appendParam(StringBuilder qs, String name, String value) {
        if (value == null || value.isEmpty()) {
            return;
        }
        qs.append(qs.length() == 0 ? '?' : '&').append(name).append('=').append(value);
    }
}

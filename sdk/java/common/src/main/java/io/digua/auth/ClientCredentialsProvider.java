package io.digua.auth;

import io.digua.SdkConfig;
import io.digua.AuthException;
import io.digua.SdkException;
import io.digua.HttpClientWrapper;
import io.digua.HttpRequest;
import io.digua.HttpResponse;
import io.digua.TokenInfo;

import java.util.LinkedHashMap;
import java.util.Map;

/**
 * client_credentials 流程，调用 /v1/tokens/access（JSON body）。
 */
public class ClientCredentialsProvider implements CredentialProvider {
    private final SdkConfig config;
    private final HttpClientWrapper http;
    private final String clientId;
    private final String clientSecret;
    private final String code;
    private final String tokenPath;

    public ClientCredentialsProvider(SdkConfig config, String clientId, String clientSecret, String code) {
        this(config, clientId, clientSecret, code, "/v1/tokens/access");
    }

    public ClientCredentialsProvider(SdkConfig config, String clientId, String clientSecret, String code, String tokenPath) {
        this.config = config;
        this.http = new HttpClientWrapper(config);
        this.clientId = clientId;
        this.clientSecret = clientSecret;
        this.code = code;
        this.tokenPath = tokenPath;
    }

    @Override
    public TokenInfo fetchToken() {
        Map<String, Object> payload = new LinkedHashMap<>();
        payload.put("grant_type", "authorization_code");
        payload.put("code", code);
        payload.put("client_id", clientId);
        payload.put("client_secret", clientSecret);
        HttpRequest req = HttpRequest.post(config.getBaseUrl() + tokenPath)
                .jsonBody(payload);
        HttpResponse resp = http.execute(req);
        if (resp.getStatusCode() == 401 || resp.getStatusCode() == 403) {
            throw new AuthException("client_credentials failed", resp.getStatusCode(), resp.getBody());
        }
        if (resp.getStatusCode() < 200 || resp.getStatusCode() >= 300) {
            throw new SdkException("client_credentials failed: HTTP " + resp.getStatusCode(), resp.getStatusCode(), resp.getBody(), null);
        }
        TokenInfo t = com.digua.sdk.common.JsonUtil.fromJson(resp.getBody(), TokenInfo.class);
        if (t == null || t.getAccessToken() == null) {
            throw new SdkException("token response missing access_token", resp.getStatusCode(), resp.getBody(), null);
        }
        return t;
    }

    @Override
    public void invalidate() {
        // 无客户端状态可清
    }
}

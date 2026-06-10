package com.kudi.coder.sdk.common.auth;

import com.kudi.coder.sdk.common.SdkConfig;
import com.kudi.coder.sdk.common.exception.AuthException;
import com.kudi.coder.sdk.common.exception.SdkException;
import com.kudi.coder.sdk.common.http.HttpClientWrapper;
import com.kudi.coder.sdk.common.http.HttpRequest;
import com.kudi.coder.sdk.common.http.HttpResponse;
import com.kudi.coder.sdk.common.model.TokenInfo;

import java.util.LinkedHashMap;
import java.util.Map;

/**
 * client_credentials 流程，调用 /api/tokens/access。
 */
public class ClientCredentialsProvider implements CredentialProvider {
    private final SdkConfig config;
    private final HttpClientWrapper http;
    private final String clientId;
    private final String clientSecret;
    private final String code;
    private final String tokenPath;

    public ClientCredentialsProvider(SdkConfig config, String clientId, String clientSecret, String code) {
        this(config, clientId, clientSecret, code, "/api/tokens/access");
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
        Map<String, String> form = new LinkedHashMap<>();
        form.put("grant_type", "authorization_code");
        form.put("code", code);
        form.put("client_id", clientId);
        form.put("client_secret", clientSecret);
        StringBuilder body = new StringBuilder();
        for (Map.Entry<String, String> e : form.entrySet()) {
            if (body.length() > 0) body.append('&');
            try {
                body.append(e.getKey()).append('=').append(java.net.URLEncoder.encode(e.getValue(), "UTF-8"));
            } catch (java.io.UnsupportedEncodingException ex) {
                throw new SdkException("URL encoding failed", ex);
            }
        }
        HttpRequest req = HttpRequest.post(config.getBaseUrl() + tokenPath)
                .body(body.toString(), "application/x-www-form-urlencoded");
        HttpResponse resp = http.execute(req);
        if (resp.getStatusCode() == 401 || resp.getStatusCode() == 403) {
            throw new AuthException("client_credentials failed", resp.getStatusCode(), resp.getBody());
        }
        if (resp.getStatusCode() < 200 || resp.getStatusCode() >= 300) {
            throw new SdkException("client_credentials failed: HTTP " + resp.getStatusCode(), resp.getStatusCode(), resp.getBody(), null);
        }
        TokenInfo t = com.kudi.coder.sdk.common.JsonUtil.fromJson(resp.getBody(), TokenInfo.class);
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

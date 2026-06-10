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
 * 用户名/密码登录。调用 /api/v1/auth/login，期望返回 {access_token, refresh_token, expires_in}。
 */
public class PasswordCredentialProvider implements CredentialProvider {
    private final SdkConfig config;
    private final HttpClientWrapper http;
    private final String username;
    private final String password;
    private final String loginPath;
    private volatile TokenInfo lastToken;

    public PasswordCredentialProvider(SdkConfig config, String username, String password) {
        this(config, username, password, "/api/v1/auth/login");
    }

    public PasswordCredentialProvider(SdkConfig config, String username, String password, String loginPath) {
        this.config = config;
        this.http = new HttpClientWrapper(config);
        this.username = username;
        this.password = password;
        this.loginPath = loginPath;
    }

    @Override
    public TokenInfo fetchToken() {
        Map<String, String> form = new LinkedHashMap<>();
        form.put("username", username);
        form.put("password", password);
        StringBuilder body = new StringBuilder();
        for (Map.Entry<String, String> e : form.entrySet()) {
            if (body.length() > 0) body.append('&');
            try {
                body.append(e.getKey()).append('=').append(java.net.URLEncoder.encode(e.getValue(), "UTF-8"));
            } catch (java.io.UnsupportedEncodingException ex) {
                throw new SdkException("URL encoding failed", ex);
            }
        }
        HttpRequest req = HttpRequest.post(config.getBaseUrl() + loginPath)
                .body(body.toString(), "application/x-www-form-urlencoded");
        HttpResponse resp = http.execute(req);
        if (resp.getStatusCode() == 401 || resp.getStatusCode() == 403) {
            throw new AuthException("login failed", resp.getStatusCode(), resp.getBody());
        }
        if (resp.getStatusCode() < 200 || resp.getStatusCode() >= 300) {
            throw new SdkException("login failed: HTTP " + resp.getStatusCode(), resp.getStatusCode(), resp.getBody(), null);
        }
        TokenInfo t = com.kudi.coder.sdk.common.JsonUtil.fromJson(resp.getBody(), TokenInfo.class);
        if (t == null || t.getAccessToken() == null) {
            throw new SdkException("login response missing access_token", resp.getStatusCode(), resp.getBody(), null);
        }
        lastToken = t;
        return t;
    }

    @Override
    public void invalidate() {
        // 登录会话无服务端状态可吊销；这里清空 lastToken 让下次重新登录
        lastToken = null;
    }
}

package com.kudi.coder.sdk.common.auth;

import com.kudi.coder.sdk.common.model.TokenInfo;

/**
 * 外部直接注入 access_token。
 */
public class StaticTokenProvider implements CredentialProvider {
    private volatile String accessToken;
    private final long expiresInSec;

    public StaticTokenProvider(String accessToken, long expiresInSec) {
        this.accessToken = accessToken;
        this.expiresInSec = expiresInSec;
    }

    @Override
    public TokenInfo fetchToken() {
        TokenInfo t = new TokenInfo();
        t.setAccessToken(accessToken);
        t.setExpiresIn(expiresInSec);
        t.setTokenType("Bearer");
        return t;
    }

    @Override
    public void invalidate() {
        // 外部 token 不在 SDK 失效
    }

    public void setAccessToken(String accessToken) {
        this.accessToken = accessToken;
    }
}

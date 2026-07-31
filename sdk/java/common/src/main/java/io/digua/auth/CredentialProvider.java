package io.digua.auth;

import io.digua.TokenInfo;

/**
 * 提供 access_token 的来源。
 * 实现可以是用户名/密码、api_key/secret，或外部注入。
 */
public interface CredentialProvider {
    /**
     * 拉取或刷新 access_token。
     * 失败时抛 AuthException。
     */
    TokenInfo fetchToken();

    /**
     * 强制失效当前缓存的 token，下次 fetchToken() 将重新签发。
     */
    void invalidate();
}

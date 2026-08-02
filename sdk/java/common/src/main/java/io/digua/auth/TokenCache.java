package io.digua.auth;

import io.digua.model.TokenInfo;

import java.util.concurrent.locks.ReentrantLock;

/**
 * 线程安全的 token 缓存。
 * - 提前 60s 主动刷新
 * - 收到 401 时调用 invalidate() 强制重拉
 */
public class TokenCache {
    private static final long REFRESH_SKEW_MS = 60_000L;

    private final CredentialProvider provider;
    private final ReentrantLock lock = new ReentrantLock();
    private volatile TokenInfo cached;
    private volatile long expiresAtMs;

    public TokenCache(CredentialProvider provider) {
        this.provider = provider;
    }

    public String getAccessToken() {
        TokenInfo t = cached;
        if (t != null && System.currentTimeMillis() < expiresAtMs - REFRESH_SKEW_MS) {
            return t.getAccessToken();
        }
        lock.lock();
        try {
            if (cached == null || System.currentTimeMillis() >= expiresAtMs - REFRESH_SKEW_MS) {
                cached = provider.fetchToken();
                if (cached == null || cached.getAccessToken() == null) {
                    throw new IllegalStateException("credential provider returned empty token");
                }
                expiresAtMs = System.currentTimeMillis() + cached.getExpiresIn() * 1000L;
            }
            return cached.getAccessToken();
        } finally {
            lock.unlock();
        }
    }

    public void invalidate() {
        lock.lock();
        try {
            cached = null;
            expiresAtMs = 0;
            provider.invalidate();
        } finally {
            lock.unlock();
        }
    }
}

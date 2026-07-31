package io.digua.common;

import io.digua.CredentialProvider;
import io.digua.TokenCache;
import io.digua.TokenInfo;
import org.junit.jupiter.api.Test;

import java.util.concurrent.atomic.AtomicInteger;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertNotEquals;

class TokenCacheTest {
    @Test
    void cachesUntilCloseToExpiry() {
        AtomicInteger calls = new AtomicInteger();
        TokenInfo t = new TokenInfo();
        t.setAccessToken("a-1");
        t.setExpiresIn(120);
        CredentialProvider p = new CredentialProvider() {
            @Override public TokenInfo fetchToken() {
                calls.incrementAndGet();
                return t;
            }
            @Override public void invalidate() {}
        };
        TokenCache cache = new TokenCache(p);
        assertEquals("a-1", cache.getAccessToken());
        assertEquals("a-1", cache.getAccessToken());
        assertEquals(1, calls.get());
    }

    @Test
    void invalidateForcesRefresh() {
        AtomicInteger calls = new AtomicInteger();
        CredentialProvider p = new CredentialProvider() {
            int n = 0;
            @Override public TokenInfo fetchToken() {
                calls.incrementAndGet();
                TokenInfo t = new TokenInfo();
                t.setAccessToken("a-" + (++n));
                t.setExpiresIn(120);
                return t;
            }
            @Override public void invalidate() {}
        };
        TokenCache cache = new TokenCache(p);
        String a = cache.getAccessToken();
        cache.invalidate();
        String b = cache.getAccessToken();
        assertEquals("a-1", a);
        assertEquals("a-2", b);
        assertNotEquals(a, b);
    }
}

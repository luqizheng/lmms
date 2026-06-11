package com.kudi.coder.httpclient;

import java.time.Duration;
import java.util.UUID;
import java.util.concurrent.atomic.AtomicInteger;
import org.junit.jupiter.api.Test;
import com.kudi.coder.httpclient.Models.TokenInfo;
import static org.junit.jupiter.api.Assertions.*;

class ServiceTokenCacheTest {

    private static TokenInfo info() {
        TokenInfo i = new TokenInfo();
        i.accessToken = "tok";
        i.expiresIn = 3600;
        return i;
    }

    private static String uniqueCid() {
        return "cid-" + UUID.randomUUID();
    }

    @Test
    void cachesPerCredential() throws Exception {
        var cache = new ServiceTokenCache(Duration.ofSeconds(60));
        String cid = uniqueCid();
        AtomicInteger calls = new AtomicInteger();
        var t1 = cache.getOrFetch(cid, "s1", () -> { calls.incrementAndGet(); return info(); });
        var t2 = cache.getOrFetch(cid, "s1", () -> { calls.incrementAndGet(); return info(); });
        assertEquals("tok", t1);
        assertEquals("tok", t2);
        assertEquals(1, calls.get());
    }

    @Test
    void separateCredentials() throws Exception {
        var cache = new ServiceTokenCache(Duration.ofSeconds(60));
        AtomicInteger calls = new AtomicInteger();
        cache.getOrFetch(uniqueCid(), "s1", () -> { calls.incrementAndGet(); return info(); });
        cache.getOrFetch(uniqueCid(), "s1", () -> { calls.incrementAndGet(); return info(); });
        assertEquals(2, calls.get());
        assertTrue(cache.size() >= 2);
    }

    @Test
    void invalidateRefetches() throws Exception {
        var cache = new ServiceTokenCache(Duration.ofSeconds(60));
        String cid = uniqueCid();
        AtomicInteger calls = new AtomicInteger();
        cache.getOrFetch(cid, "s1", () -> { calls.incrementAndGet(); return info(); });
        cache.getOrFetch(cid, "s1", () -> { calls.incrementAndGet(); return info(); });
        assertEquals(1, calls.get());
        cache.invalidate(cid, "s1");
        cache.getOrFetch(cid, "s1", () -> { calls.incrementAndGet(); return info(); });
        assertEquals(2, calls.get());
    }
}

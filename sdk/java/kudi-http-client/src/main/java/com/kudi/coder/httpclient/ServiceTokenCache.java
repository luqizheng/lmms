package com.kudi.coder.httpclient;

import java.nio.charset.StandardCharsets;
import java.security.MessageDigest;
import java.time.Duration;
import java.time.Instant;
import java.util.concurrent.ConcurrentHashMap;
import java.util.concurrent.locks.ReentrantLock;
import com.kudi.coder.httpclient.Models.TokenInfo;

public class ServiceTokenCache {
    private static final ConcurrentHashMap<String, Entry> STORE = new ConcurrentHashMap<>();
    private final Duration refreshSkew;
    private final ReentrantLock lock = new ReentrantLock();

    public ServiceTokenCache(Duration refreshSkew) {
        this.refreshSkew = refreshSkew;
    }

    public int size() {
        return STORE.size();
    }

    public String getOrFetch(String clientId, String secretKey, ThrowingSupplier<TokenInfo> fetcher) throws Exception {
        String key = digest(clientId, secretKey);
        Entry hit = STORE.get(key);
        if (hit != null && hit.matches(clientId, secretKey)
                && hit.expiresAt.isAfter(Instant.now().plus(refreshSkew))) {
            return hit.info.accessToken;
        }
        lock.lock();
        try {
            hit = STORE.get(key);
            if (hit != null && hit.matches(clientId, secretKey)
                    && hit.expiresAt.isAfter(Instant.now().plus(refreshSkew))) {
                return hit.info.accessToken;
            }
            TokenInfo info = fetcher.get();
            STORE.put(key, new Entry(
                    clientId,
                    secretKey,
                    info,
                    Instant.now().plusSeconds(Math.max(0, info.expiresIn))));
            return info.accessToken;
        } finally {
            lock.unlock();
        }
    }

    public void invalidate(String clientId, String secretKey) {
        STORE.remove(digest(clientId, secretKey));
    }

    private static String digest(String a, String b) {
        try {
            MessageDigest md = MessageDigest.getInstance("SHA-256");
            byte[] h = md.digest((a + "|" + b).getBytes(StandardCharsets.UTF_8));
            StringBuilder sb = new StringBuilder();
            for (byte x : h) sb.append(String.format("%02X", x));
            return sb.toString();
        } catch (Exception e) {
            throw new RuntimeException(e);
        }
    }

    @FunctionalInterface
    public interface ThrowingSupplier<T> {
        T get() throws Exception;
    }

    private static class Entry {
        final String clientId, secretKey;
        final TokenInfo info;
        final Instant expiresAt;

        Entry(String cid, String sk, TokenInfo i, Instant e) {
            clientId = cid;
            secretKey = sk;
            info = i;
            expiresAt = e;
        }

        boolean matches(String cid, String sk) {
            return clientId.equals(cid) && secretKey.equals(sk);
        }
    }
}

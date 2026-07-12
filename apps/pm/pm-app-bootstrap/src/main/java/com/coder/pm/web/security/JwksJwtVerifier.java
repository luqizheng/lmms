package com.coder.pm.web.security;

import com.coder.pm.domain.security.Principal;
import com.fasterxml.jackson.databind.JsonNode;
import com.fasterxml.jackson.databind.ObjectMapper;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.springframework.beans.factory.annotation.Value;
import org.springframework.stereotype.Component;

import java.io.IOException;
import java.math.BigInteger;
import java.net.URI;
import java.net.http.HttpClient;
import java.net.http.HttpRequest;
import java.net.http.HttpResponse;
import java.security.KeyFactory;
import java.security.PublicKey;
import java.security.Signature;
import java.security.interfaces.RSAPublicKey;
import java.security.spec.RSAPublicKeySpec;
import java.time.Duration;
import java.time.Instant;
import java.util.ArrayList;
import java.util.Base64;
import java.util.HashMap;
import java.util.List;
import java.util.Map;
import java.util.UUID;
import java.util.concurrent.atomic.AtomicReference;

/**
 * Verifies RS256 JWTs issued by the iam-service / token-service pair.
 *
 * <p>The verifier pulls the public keys from the JWKS document at
 * {@code pm.jwt.jwks-url} (default
 * {@code http://localhost:3000/v1/tokens/.well-known/jwks.json}), parses the
 * JWK with {@code kty=RSA & alg=RS256}, picks the JWK whose {@code kid}
 * matches the JWT header, then validates the signature with {@code KeyFactory}.
 *
 * <p>Issuer + audience + scope are configurable; expired or malformed tokens
 * are rejected by returning {@link Principal#ANONYMOUS}.
 *
 * <p>Thread-safe: the parsed key cache is a single {@link AtomicReference} and
 * is refreshed at most every {@code cacheTtlMinutes} (default 10) or when the
 * current request needs a {@code kid} not present in the cache.
 *
 * <p>NOT annotated with {@code @Component} — instantiation is owned by
 * {@link JwtVerifierRouter} so we don't end up with multiple {@link JwtVerifier}
 * beans competing for autowiring.
 */
public class JwksJwtVerifier implements JwtVerifier {

    private static final Logger log = LoggerFactory.getLogger(JwksJwtVerifier.class);
    private static final Duration HTTP_TIMEOUT = Duration.ofSeconds(5);
    private static final Duration CONNECT_TIMEOUT = Duration.ofSeconds(2);

    private final String jwksUrl;
    private final String issuer;
    private final String audience;
    private final String requiredScope;
    private final long cacheTtlMinutes;
    private final ObjectMapper mapper;
    private final HttpClient http;
    private final AtomicReference<CacheEntry> cache = new AtomicReference<>();

    public JwksJwtVerifier(
            @Value("${pm.jwt.jwks-url:http://localhost:3000/v1/tokens/.well-known/jwks.json}") String jwksUrl,
            @Value("${pm.jwt.issuer:superpowers-token-service}") String issuer,
            @Value("${pm.jwt.audience:coder-micro-services}") String audience,
            @Value("${pm.jwt.required-scope:access}") String requiredScope,
            @Value("${pm.jwt.cache-ttl-minutes:10}") long cacheTtlMinutes) {
        this.jwksUrl = jwksUrl;
        this.issuer = issuer;
        this.audience = audience;
        this.requiredScope = requiredScope;
        this.cacheTtlMinutes = cacheTtlMinutes;
        this.mapper = new ObjectMapper();
        this.http = HttpClient.newBuilder()
                .connectTimeout(CONNECT_TIMEOUT)
                .build();
        log.info("JwksJwtVerifier: jwks-url={} issuer={} audience={} required-scope={} cache-ttl-min={}",
                jwksUrl, issuer, audience, requiredScope, cacheTtlMinutes);
    }

    @Override
    public Principal verify(String token) {
        if (token == null || token.isBlank()) {
            return Principal.ANONYMOUS;
        }
        String[] parts = token.split("\\.");
        if (parts.length != 3) {
            log.debug("Malformed JWT: expected 3 segments, got {}", parts.length);
            return Principal.ANONYMOUS;
        }
        JsonNode header;
        JsonNode payload;
        try {
            header = mapper.readTree(decodeSegment(parts[0]));
            payload = mapper.readTree(decodeSegment(parts[1]));
        } catch (Exception ex) {
            log.debug("JWT decode failed: {}", ex.getMessage());
            return Principal.ANONYMOUS;
        }

        String alg = textOrNull(header, "alg");
        String kid = textOrNull(header, "kid");
        if (!"RS256".equals(alg)) {
            log.debug("Unexpected JWT alg: {}", alg);
            return Principal.ANONYMOUS;
        }
        if (kid == null) {
            log.debug("JWT header missing kid");
            return Principal.ANONYMOUS;
        }

        // exp + iss + aud + scope checks before crypto
        Long exp = longOrNull(payload, "exp");
        if (exp == null || Instant.now().getEpochSecond() >= exp) {
            log.debug("JWT expired or missing exp");
            return Principal.ANONYMOUS;
        }
        if (!issuer.equals(textOrNull(payload, "iss"))) {
            log.debug("JWT issuer mismatch");
            return Principal.ANONYMOUS;
        }
        if (audience != null && !audience.isBlank()
                && !audienceMatches(payload.get("aud"), audience)) {
            log.debug("JWT audience mismatch");
            return Principal.ANONYMOUS;
        }
        if (requiredScope != null && !requiredScope.isBlank()
                && !requiredScope.equals(textOrNull(payload, "scope"))) {
            log.debug("JWT scope mismatch (expected {}, got {})", requiredScope, textOrNull(payload, "scope"));
            return Principal.ANONYMOUS;
        }

        // Verify signature with the JWK matching kid
        PublicKey key = findKey(kid);
        if (key == null) {
            // cache miss → force one refresh then retry
            refreshCache(true);
            key = findKey(kid);
            if (key == null) {
                log.debug("No JWK matching kid={} after refresh", kid);
                return Principal.ANONYMOUS;
            }
        }
        if (!verifySignature(parts[0], parts[1], parts[2], key)) {
            log.debug("JWT signature verification failed (kid={})", kid);
            return Principal.ANONYMOUS;
        }

        Long userId = parseUserId(textOrNull(payload, "sub"));
        String username = textOrNull(payload, "user_code");
        Long tenantId = longOrNull(payload, "tid");
        List<String> roles = stringListOrEmpty(payload, "roles");
        List<String> permissions = stringListOrEmpty(payload, "permissions");
        String scope = textOrNull(payload, "scope");

        return new Principal(userId, username, tenantId, roles, permissions, scope);
    }

    // ---------- key cache ----------

    private PublicKey findKey(String kid) {
        CacheEntry entry = cache.get();
        if (entry != null && !entry.expired(cacheTtlMinutes)) {
            PublicKey k = entry.keys.get(kid);
            if (k != null) return k;
        }
        // expired OR kid not found → refresh once
        refreshCache(false);
        CacheEntry refreshed = cache.get();
        return refreshed == null ? null : refreshed.keys.get(kid);
    }

    private synchronized void refreshCache(boolean forced) {
        CacheEntry current = cache.get();
        if (!forced && current != null && !current.expired(cacheTtlMinutes)) {
            return;
        }
        try {
            HttpRequest req = HttpRequest.newBuilder()
                    .uri(URI.create(jwksUrl))
                    .timeout(HTTP_TIMEOUT)
                    .header("Accept", "application/json")
                    .GET()
                    .build();
            HttpResponse<String> resp = http.send(req, HttpResponse.BodyHandlers.ofString());
            if (resp.statusCode() / 100 != 2) {
                log.warn("JWKS endpoint {} returned HTTP {}", jwksUrl, resp.statusCode());
                return;
            }
            JsonNode root = mapper.readTree(resp.body());
            JsonNode keysNode = root.get("keys");
            if (keysNode == null || !keysNode.isArray()) {
                log.warn("JWKS payload missing 'keys' array");
                return;
            }
            Map<String, PublicKey> parsed = new HashMap<>();
            for (JsonNode jwk : keysNode) {
                if (!"RSA".equals(textOrNull(jwk, "kty"))) continue;
                String alg = textOrNull(jwk, "alg");
                if (alg != null && !"RS256".equals(alg)) continue;
                String jwkKid = textOrNull(jwk, "kid");
                if (jwkKid == null) continue;
                BigInteger n = new BigInteger(1, Base64.getUrlDecoder().decode(textOrNull(jwk, "n")));
                BigInteger e = new BigInteger(1, Base64.getUrlDecoder().decode(textOrNull(jwk, "e")));
                RSAPublicKeySpec spec = new RSAPublicKeySpec(n, e);
                PublicKey pk = KeyFactory.getInstance("RSA").generatePublic(spec);
                parsed.put(jwkKid, pk);
            }
            if (parsed.isEmpty()) {
                log.warn("JWKS contained no usable RSA keys");
                return;
            }
            cache.set(new CacheEntry(parsed, Instant.now()));
            log.info("JWKS refreshed from {}: {} key(s) loaded", jwksUrl, parsed.size());
        } catch (IOException | InterruptedException ex) {
            if (ex instanceof InterruptedException) Thread.currentThread().interrupt();
            log.warn("JWKS refresh from {} failed: {}", jwksUrl, ex.getMessage());
        } catch (Exception ex) {
            log.warn("JWKS parse error: {}", ex.getMessage());
        }
    }

    private static boolean verifySignature(String headerSeg, String payloadSeg, String sigSeg, PublicKey key) {
        try {
            byte[] data = (headerSeg + "." + payloadSeg).getBytes(java.nio.charset.StandardCharsets.US_ASCII);
            byte[] sig = Base64.getUrlDecoder().decode(sigSeg);
            Signature verifier = Signature.getInstance("SHA256withRSA");
            verifier.initVerify(key);
            verifier.update(data);
            return verifier.verify(sig);
        } catch (Exception ex) {
            return false;
        }
    }

    private static byte[] decodeSegment(String seg) {
        return Base64.getUrlDecoder().decode(seg);
    }

    private static String textOrNull(JsonNode node, String field) {
        JsonNode v = node == null ? null : node.get(field);
        return v == null || v.isNull() ? null : v.asText();
    }

    private static Long longOrNull(JsonNode node, String field) {
        JsonNode v = node == null ? null : node.get(field);
        if (v == null || v.isNull() || !v.isNumber()) return null;
        return v.asLong();
    }

    private static List<String> stringListOrEmpty(JsonNode node, String field) {
        JsonNode v = node == null ? null : node.get(field);
        if (v == null || !v.isArray()) return List.of();
        List<String> out = new ArrayList<>(v.size());
        v.forEach(n -> out.add(n.asText()));
        return out;
    }

    private static boolean audienceMatches(JsonNode audClaim, String expected) {
        if (audClaim == null || audClaim.isNull()) return false;
        if (audClaim.isTextual()) return expected.equals(audClaim.asText());
        if (audClaim.isArray()) {
            for (JsonNode a : audClaim) {
                if (a.isTextual() && expected.equals(a.asText())) return true;
            }
        }
        return false;
    }

    /**
     * Convert the user-id sub claim (UUID string in iam-service) to a long.
     * Strategy: strip hyphens, take first 16 hex chars → parse as unsigned hex.
     * Returns null when the result overflows long or the input is not UUID-shaped.
     */
    private static Long parseUserId(String sub) {
        if (sub == null) return null;
        try {
            // Hex long from the first 8 bytes of the UUID (most significant bits).
            // UUID "550e8400-e29b-41d4-a716-446655440000" → 0x550e8400e29b41d4
            String hex = sub.replace("-", "");
            if (hex.length() < 16) return null;
            return Long.parseUnsignedLong(hex.substring(0, 16), 16);
        } catch (NumberFormatException ex) {
            return null;
        }
    }

    // For tests: expose UUID→long for stable identity comparison.
    static Long parseUserIdForTest(String sub) {
        return parseUserId(sub);
    }

    // Quick sanity check that a UUID string parses as expected.
    @SuppressWarnings("unused")
    private static String asUuid(long first8Bytes) {
        return new UUID(first8Bytes, 0L).toString();
    }

    private static final class CacheEntry {
        final Map<String, PublicKey> keys;
        final Instant fetchedAt;
        CacheEntry(Map<String, PublicKey> keys, Instant fetchedAt) {
            this.keys = keys;
            this.fetchedAt = fetchedAt;
        }
        boolean expired(long ttlMinutes) {
            return fetchedAt.isBefore(Instant.now().minus(Duration.ofMinutes(ttlMinutes)));
        }
    }
}
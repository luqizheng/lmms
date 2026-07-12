package com.coder.pm.web.security;

import com.fasterxml.jackson.databind.JsonNode;
import com.fasterxml.jackson.databind.ObjectMapper;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.springframework.beans.factory.annotation.Value;
import org.springframework.stereotype.Component;

import javax.crypto.Mac;
import javax.crypto.spec.SecretKeySpec;
import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.Base64;
import java.util.List;

/**
 * HS256 verifier used as a local-dev fallback.
 *
 * <p>This is intentionally <strong>not</strong> a production verifier: HS256
 * with a shared secret means anyone with the secret can mint valid tokens.
 * Use {@link JwksJwtVerifier} (the default in any environment that should
 * talk to real iam-service) when you need real authentication.
 *
 * <p>Activated only when {@code pm.jwt.mode=hs256} (or the dev profile's
 * default). Subject parsing is liberal — {@code sub} is taken as a Long,
 * but if that fails it falls back to {@code userId=null} so the rest of the
 * claims still surface.
 *
 * <p>NOT annotated with {@code @Component} — instantiation is owned by
 * {@link JwtVerifierRouter} so we don't end up with multiple {@link JwtVerifier}
 * beans competing for autowiring.
 */
public class Hs256JwtVerifier implements JwtVerifier {

    private static final Logger log = LoggerFactory.getLogger(Hs256JwtVerifier.class);
    private static final String ALG = "HS256";

    private final byte[] secret;
    private final String expectedIssuer;
    private final String requiredScope;
    private final ObjectMapper mapper = new ObjectMapper();

    public Hs256JwtVerifier(
            @Value("${pm.jwt.hmac-secret:}") String hmacSecret,
            @Value("${pm.jwt.issuer:superpowers-token-service}") String expectedIssuer,
            @Value("${pm.jwt.required-scope:access}") String requiredScope) {
        this.secret = hmacSecret == null ? new byte[0] : hmacSecret.getBytes(StandardCharsets.UTF_8);
        this.expectedIssuer = expectedIssuer;
        this.requiredScope = requiredScope;
        if (secret.length == 0) {
            log.warn("Hs256JwtVerifier: hmac-secret is empty; all verify() calls will fail. " +
                    "Set pm.jwt.hmac-secret (env PM_JWT_HMAC_SECRET) for local dev.");
        } else if (secret.length < 32) {
            log.warn("Hs256JwtVerifier: hmac-secret is only {} bytes; HS256 recommends >=32. " +
                    "This verifier still accepts it.", secret.length);
        } else {
            log.info("Hs256JwtVerifier: dev verifier ready ({} bytes, issuer={}, scope={})",
                    secret.length, expectedIssuer, requiredScope);
        }
    }

    @Override
    public Principal verify(String token) {
        if (secret.length == 0 || token == null || token.isBlank()) {
            return Principal.ANONYMOUS;
        }
        String[] parts = token.split("\\.");
        if (parts.length != 3) return Principal.ANONYMOUS;
        try {
            String signingInput = parts[0] + "." + parts[1];
            byte[] expected = sign(signingInput);
            byte[] given = Base64.getUrlDecoder().decode(parts[2]);
            if (!constantTimeEquals(expected, given)) return Principal.ANONYMOUS;

            JsonNode header = mapper.readTree(Base64.getUrlDecoder().decode(parts[0]));
            if (!ALG.equals(textOr(header, "alg"))) return Principal.ANONYMOUS;

            JsonNode payload = mapper.readTree(Base64.getUrlDecoder().decode(parts[1]));
            String iss = textOr(payload, "iss");
            if (expectedIssuer != null && !expectedIssuer.isBlank() && !expectedIssuer.equals(iss)) {
                return Principal.ANONYMOUS;
            }
            if (requiredScope != null && !requiredScope.isBlank()
                    && !requiredScope.equals(textOr(payload, "scope"))) {
                return Principal.ANONYMOUS;
            }
            Long exp = longOrNull(payload, "exp");
            if (exp == null || exp <= java.time.Instant.now().getEpochSecond()) {
                return Principal.ANONYMOUS;
            }

            Long userId = parseLongOrNull(textOr(payload, "sub"));
            String username = textOr(payload, "user_code");
            if (username == null) username = textOr(payload, "uname");
            Long tenantId = longOrNull(payload, "tid");
            return new Principal(
                    userId, username, tenantId,
                    stringList(payload, "roles"),
                    stringList(payload, "permissions"),
                    textOr(payload, "scope"));
        } catch (Exception ex) {
            log.debug("HS256 verify failed: {}", ex.getMessage());
            return Principal.ANONYMOUS;
        }
    }

    private byte[] sign(String input) throws Exception {
        Mac mac = Mac.getInstance("HmacSHA256");
        mac.init(new SecretKeySpec(secret, "HmacSHA256"));
        return mac.doFinal(input.getBytes(StandardCharsets.US_ASCII));
    }

    private static boolean constantTimeEquals(byte[] a, byte[] b) {
        if (a.length != b.length) return false;
        int diff = 0;
        for (int i = 0; i < a.length; i++) diff |= a[i] ^ b[i];
        return diff == 0;
    }

    private static String textOr(JsonNode node, String field) {
        JsonNode v = node == null ? null : node.get(field);
        return v == null || v.isNull() ? null : v.asText();
    }

    private static Long longOrNull(JsonNode node, String field) {
        JsonNode v = node == null ? null : node.get(field);
        return v == null || !v.isNumber() ? null : v.asLong();
    }

    private static Long parseLongOrNull(String s) {
        if (s == null || s.isBlank()) return null;
        // First try decimal — dev tokens often use plain numeric sub.
        try { return Long.parseLong(s); } catch (NumberFormatException ignored) {}
        // Then try UUID hex (first 16 chars after dash-strip) — matches JwksJwtVerifier strategy.
        try {
            String hex = s.replace("-", "");
            if (hex.length() < 16) return null;
            return Long.parseUnsignedLong(hex.substring(0, 16), 16);
        } catch (NumberFormatException ignored) { return null; }
    }

    private static List<String> stringList(JsonNode node, String field) {
        JsonNode v = node == null ? null : node.get(field);
        if (v == null || !v.isArray()) return List.of();
        List<String> out = new ArrayList<>(v.size());
        v.forEach(n -> out.add(n.asText()));
        return out;
    }
}
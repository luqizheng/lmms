package com.coder.pm.web.security;

import com.coder.pm.domain.security.Principal;
import org.junit.jupiter.api.Test;

import javax.crypto.Mac;
import javax.crypto.spec.SecretKeySpec;
import java.nio.charset.StandardCharsets;
import java.time.Instant;
import java.util.Base64;
import java.util.List;
import java.util.UUID;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertTrue;

class Hs256JwtVerifierTest {

    private static final String SECRET = "dev-hs256-secret-shared-with-iam-stub";

    @Test
    void rejectsWhenSecretEmpty() {
        Hs256JwtVerifier v = new Hs256JwtVerifier("", "iss", "access");
        assertTrue(v.verify("any.token.here").isAnonymous());
    }

    @Test
    void rejectsWhenSignatureWrong() {
        Hs256JwtVerifier v = new Hs256JwtVerifier(SECRET, "iss", "access");
        long now = Instant.now().getEpochSecond();
        String headerJson = "{\"alg\":\"HS256\",\"typ\":\"JWT\"}";
        String payloadJson = "{\"iss\":\"iss\",\"aud\":\"aud\",\"sub\":\"u\",\"scope\":\"access\",\"iat\":" + now + ",\"exp\":" + (now + 60) + "}";
        String signingInput = b64u(headerJson) + "." + b64u(payloadJson);
        String bogusSig = Base64.getUrlEncoder().withoutPadding().encodeToString(new byte[]{1, 2, 3, 4});
        assertTrue(v.verify(signingInput + "." + bogusSig).isAnonymous());
    }

    @Test
    void acceptsValidHs256Token() throws Exception {
        Hs256JwtVerifier v = new Hs256JwtVerifier(SECRET, "iss", "access");
        long now = Instant.now().getEpochSecond();
        String headerJson = "{\"alg\":\"HS256\",\"typ\":\"JWT\"}";
        var payload = new java.util.LinkedHashMap<String, Object>();
        payload.put("iss", "iss");
        payload.put("aud", "aud");
        payload.put("sub", "12345");
        payload.put("user_code", "alice");
        payload.put("tid", 42L);
        payload.put("roles", List.of("pm_admin"));
        payload.put("permissions", List.of("pm_project_create"));
        payload.put("scope", "access");
        payload.put("iat", now);
        payload.put("exp", now + 600);
        payload.put("jti", UUID.randomUUID().toString());
        String payloadJson = new com.fasterxml.jackson.databind.ObjectMapper().writeValueAsString(payload);
        String signingInput = b64u(headerJson) + "." + b64u(payloadJson);
        String sig = sign(SECRET, signingInput);
        Principal p = v.verify(signingInput + "." + sig);
        assertNotNull(p);
        assertEquals(12345L, p.userId());
        assertEquals("alice", p.username());
        assertEquals(42L, p.tenantId());
        assertTrue(p.hasRole("pm_admin"));
    }

    @Test
    void rejectsWrongIssuerAndScope() throws Exception {
        Hs256JwtVerifier v = new Hs256JwtVerifier(SECRET, "expected-iss", "access");
        long now = Instant.now().getEpochSecond();
        String token = signWithClaims(SECRET, "different-iss", "aud", "u", "scope", "refresh", now, now + 60);
        assertTrue(v.verify(token).isAnonymous());
        String token2 = signWithClaims(SECRET, "expected-iss", "aud", "u", "scope", "access", now, now + 60);
        // accepted (signature + iss + scope ok); sub="u" is not a decimal nor a UUID → userId null,
        // but username "scope" should survive
        Principal p2 = v.verify(token2);
        assertNotNull(p2);
        assertEquals(null, p2.userId());
        assertEquals("scope", p2.username());
    }

    @Test
    void rejectsExpired() throws Exception {
        Hs256JwtVerifier v = new Hs256JwtVerifier(SECRET, "iss", "access");
        long now = Instant.now().getEpochSecond();
        String token = signWithClaims(SECRET, "iss", "aud", "u", "scope", "access", now - 7200, now - 3600);
        assertTrue(v.verify(token).isAnonymous());
    }

    private static String signWithClaims(String secret, String iss, String aud, String sub,
                                         String uname, String scope, long iat, long exp) throws Exception {
        var payload = new java.util.LinkedHashMap<String, Object>();
        payload.put("iss", iss);
        payload.put("aud", aud);
        payload.put("sub", sub);
        payload.put("user_code", uname);
        payload.put("scope", scope);
        payload.put("iat", iat);
        payload.put("exp", exp);
        payload.put("jti", UUID.randomUUID().toString());
        String headerJson = "{\"alg\":\"HS256\",\"typ\":\"JWT\"}";
        String payloadJson = new com.fasterxml.jackson.databind.ObjectMapper().writeValueAsString(payload);
        return sign(headerJson, payloadJson, secret);
    }

    private static String sign(String headerJson, String payloadJson, String secret) throws Exception {
        String signingInput = b64u(headerJson) + "." + b64u(payloadJson);
        return signingInput + "." + sign(secret, signingInput);
    }

    private static String sign(String secret, String signingInput) throws Exception {
        Mac mac = Mac.getInstance("HmacSHA256");
        mac.init(new SecretKeySpec(secret.getBytes(StandardCharsets.UTF_8), "HmacSHA256"));
        return Base64.getUrlEncoder().withoutPadding().encodeToString(mac.doFinal(signingInput.getBytes(StandardCharsets.US_ASCII)));
    }

    private static String b64u(String s) {
        return Base64.getUrlEncoder().withoutPadding().encodeToString(s.getBytes(StandardCharsets.UTF_8));
    }
}
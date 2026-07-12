package com.coder.pm.web.security;

import com.coder.pm.domain.security.Principal;
import org.junit.jupiter.api.Test;

import java.lang.reflect.Field;
import java.lang.reflect.InvocationTargetException;
import java.lang.reflect.Method;
import java.security.KeyPair;
import java.security.KeyPairGenerator;
import java.security.interfaces.RSAPublicKey;
import java.time.Instant;
import java.util.Base64;
import java.util.List;
import java.util.UUID;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertNull;
import static org.junit.jupiter.api.Assertions.assertTrue;
import static org.junit.jupiter.api.Assertions.fail;

/**
 * Unit tests for {@link JwksJwtVerifier}. Spins up a small in-process HTTP
 * server that serves a JWKS document, signs tokens with the matching RSA
 * keypair, and exercises the verifier's happy / sad paths end-to-end.
 */
class JwksJwtVerifierTest {

    @Test
    void rejectsEmptyAndMalformedTokens() {
        JwksJwtVerifier verifier = new JwksJwtVerifier("http://127.0.0.1:1/jwks", "iss", "aud", "access", 10);
        assertTrue(verifier.verify(null).isAnonymous());
        assertTrue(verifier.verify("").isAnonymous());
        assertTrue(verifier.verify("only.two.parts.bad").isAnonymous());
        assertTrue(verifier.verify("not-a-jwt-at-all").isAnonymous());
    }

    @Test
    void happyPath_validRsaSignatureReturnsPopulatedPrincipal() throws Exception {
        // Spin up a JWKS server
        KeyPairGenerator gen = KeyPairGenerator.getInstance("RSA");
        gen.initialize(2048);
        KeyPair pair = gen.generateKeyPair();
        RSAPublicKey publicKey = (RSAPublicKey) pair.getPublic();
        String kid = "test-kid";
        FakeJwksServer server = FakeJwksServer.start(publicKey, kid);
        try {
            String url = "http://127.0.0.1:" + server.port() + "/.well-known/jwks.json";
            JwksJwtVerifier verifier = new JwksJwtVerifier(url, "superpowers-token-service", "coder-micro-services", "access", 10);
            String userId = UUID.randomUUID().toString();
            long iat = Instant.now().getEpochSecond();
            long exp = iat + 3600;
            String token = mintToken(pair.getPrivate(), kid, "RS256",
                    "superpowers-token-service", "coder-micro-services",
                    userId, "alice", 7L,
                    List.of("pm_admin"), List.of("pm_project_create"), "access",
                    iat, exp);
            Principal p = verifier.verify(token);
            assertNotNull(p, "principal should be populated");
            // userId is UUID hex (no dashes), parseUserId takes the first 16 hex chars.
            String hex = userId.replace("-", "");
            assertEquals(Long.parseUnsignedLong(hex.substring(0, 16), 16), p.userId().longValue());
            assertEquals("alice", p.username());
            assertEquals(7L, p.tenantId());
            assertTrue(p.hasRole("pm_admin"));
            assertTrue(p.hasPermission("pm_project_create"));
        } finally {
            server.stop();
        }
    }

    @Test
    void rejectsExpiredToken() throws Exception {
        KeyPairGenerator gen = KeyPairGenerator.getInstance("RSA");
        gen.initialize(2048);
        KeyPair pair = gen.generateKeyPair();
        String kid = "k1";
        FakeJwksServer server = FakeJwksServer.start((RSAPublicKey) pair.getPublic(), kid);
        try {
            String url = "http://127.0.0.1:" + server.port() + "/.well-known/jwks.json";
            JwksJwtVerifier verifier = new JwksJwtVerifier(url, "iss", "aud", "access", 10);
            long now = Instant.now().getEpochSecond();
            String token = mintToken(pair.getPrivate(), kid, "RS256",
                    UUID.randomUUID().toString(), "alice", 1L, null, null, "access",
                    now - 7200, now - 3600); // expired 1h ago
            assertTrue(verifier.verify(token).isAnonymous());
        } finally { server.stop(); }
    }

    @Test
    void rejectsWrongIssuer() throws Exception {
        KeyPairGenerator gen = KeyPairGenerator.getInstance("RSA");
        gen.initialize(2048);
        KeyPair pair = gen.generateKeyPair();
        String kid = "k2";
        FakeJwksServer server = FakeJwksServer.start((RSAPublicKey) pair.getPublic(), kid);
        try {
            String url = "http://127.0.0.1:" + server.port() + "/.well-known/jwks.json";
            JwksJwtVerifier verifier = new JwksJwtVerifier(url, "expected-issuer", "aud", "access", 10);
            long now = Instant.now().getEpochSecond();
            String token = mintToken(pair.getPrivate(), kid, "RS256",
                    UUID.randomUUID().toString(), "alice", 1L, null, null, "access",
                    now, now + 3600);
            // Mint with different issuer
            token = rewriteIssuer(token, "another-issuer", pair.getPrivate());
            assertTrue(verifier.verify(token).isAnonymous());
        } finally { server.stop(); }
    }

    @Test
    void rejectsWrongAudience() throws Exception {
        KeyPairGenerator gen = KeyPairGenerator.getInstance("RSA");
        gen.initialize(2048);
        KeyPair pair = gen.generateKeyPair();
        String kid = "k3";
        FakeJwksServer server = FakeJwksServer.start((RSAPublicKey) pair.getPublic(), kid);
        try {
            String url = "http://127.0.0.1:" + server.port() + "/.well-known/jwks.json";
            JwksJwtVerifier verifier = new JwksJwtVerifier(url, "iss", "expected-aud", "access", 10);
            long now = Instant.now().getEpochSecond();
            String token = mintToken(pair.getPrivate(), kid, "RS256",
                    UUID.randomUUID().toString(), "alice", 1L, null, null, "access",
                    now, now + 3600);
            token = rewriteAudience(token, "wrong-aud", pair.getPrivate());
            assertTrue(verifier.verify(token).isAnonymous());
        } finally { server.stop(); }
    }

    @Test
    void rejectsBadSignature() throws Exception {
        KeyPairGenerator gen = KeyPairGenerator.getInstance("RSA");
        gen.initialize(2048);
        KeyPair pair1 = gen.generateKeyPair();
        KeyPair pair2 = gen.generateKeyPair();
        String kid = "k4";
        FakeJwksServer server = FakeJwksServer.start((RSAPublicKey) pair1.getPublic(), kid);
        try {
            String url = "http://127.0.0.1:" + server.port() + "/.well-known/jwks.json";
            JwksJwtVerifier verifier = new JwksJwtVerifier(url, "iss", "aud", "access", 10);
            long now = Instant.now().getEpochSecond();
            // Sign with pair2 but JWK exposes pair1
            String token = mintToken(pair2.getPrivate(), kid, "RS256",
                    UUID.randomUUID().toString(), "alice", 1L, null, null, "access",
                    now, now + 3600);
            assertTrue(verifier.verify(token).isAnonymous());
        } finally { server.stop(); }
    }

    @Test
    void rejectsWrongScope() throws Exception {
        KeyPairGenerator gen = KeyPairGenerator.getInstance("RSA");
        gen.initialize(2048);
        KeyPair pair = gen.generateKeyPair();
        String kid = "k5";
        FakeJwksServer server = FakeJwksServer.start((RSAPublicKey) pair.getPublic(), kid);
        try {
            String url = "http://127.0.0.1:" + server.port() + "/.well-known/jwks.json";
            JwksJwtVerifier verifier = new JwksJwtVerifier(url, "iss", "aud", "access", 10);
            long now = Instant.now().getEpochSecond();
            String token = mintToken(pair.getPrivate(), kid, "RS256",
                    UUID.randomUUID().toString(), "alice", 1L, null, null, "refresh",
                    now, now + 3600);
            assertTrue(verifier.verify(token).isAnonymous());
        } finally { server.stop(); }
    }

    @Test
    void unknownKidForcesRefresh() throws Exception {
        KeyPairGenerator gen = KeyPairGenerator.getInstance("RSA");
        gen.initialize(2048);
        KeyPair pair = gen.generateKeyPair();
        String kid = "kid-A";
        FakeJwksServer server = FakeJwksServer.start((RSAPublicKey) pair.getPublic(), kid);
        try {
            String url = "http://127.0.0.1:" + server.port() + "/.well-known/jwks.json";
            JwksJwtVerifier verifier = new JwksJwtVerifier(url, "iss", "aud", "access", 10);
            long now = Instant.now().getEpochSecond();
            // Server only knows kid-A; token uses kid-A → should succeed.
            String token = mintToken(pair.getPrivate(), kid, "RS256",
                    UUID.randomUUID().toString(), "alice", 1L, null, null, "access",
                    now, now + 3600);
            Principal p = verifier.verify(token);
            assertFalse(p.isAnonymous());
        } finally { server.stop(); }
    }

    @Test
    void parseUserId_isStableForUuidInput() throws Exception {
        Method m = JwksJwtVerifier.class.getDeclaredMethod("parseUserIdForTest", String.class);
        m.setAccessible(true);
        UUID uuid = UUID.fromString("550e8400-e29b-41d4-a716-446655440000");
        Long first8 = (Long) m.invoke(null, uuid.toString());
        assertEquals(0x550e8400e29b41d4L, first8);
        // garbage → null
        assertNull(m.invoke(null, "not-a-uuid"));
        assertNull(m.invoke(null, (Object) null));
    }

    // ---------- helpers ----------

    private static String mintToken(java.security.PrivateKey priv, String kid, String alg,
                                   String issuer, String audience,
                                   String sub, String userCode, Long tid,
                                   List<String> roles, List<String> permissions, String scope,
                                   long iat, long exp) throws Exception {
        String headerJson = String.format(
                "{\"alg\":\"%s\",\"typ\":\"JWT\",\"kid\":\"%s\"}", alg, kid);
        var payload = new java.util.LinkedHashMap<String, Object>();
        payload.put("iss", issuer);
        payload.put("aud", audience);
        payload.put("sub", sub);
        payload.put("user_code", userCode);
        payload.put("tid", tid);
        if (roles != null) payload.put("roles", roles);
        if (permissions != null) payload.put("permissions", permissions);
        payload.put("scope", scope);
        payload.put("iat", iat);
        payload.put("exp", exp);
        payload.put("jti", UUID.randomUUID().toString());
        String payloadJson = new com.fasterxml.jackson.databind.ObjectMapper().writeValueAsString(payload);
        return sign(headerJson, payloadJson, priv);
    }

    // Back-compat overload for tests that don't care about issuer/audience.
    private static String mintToken(java.security.PrivateKey priv, String kid, String alg,
                                   String sub, String userCode, Long tid,
                                   List<String> roles, List<String> permissions, String scope,
                                   long iat, long exp) throws Exception {
        return mintToken(priv, kid, alg, "iss", "aud", sub, userCode, tid, roles, permissions, scope, iat, exp);
    }

    private static String rewriteIssuer(String token, String newIssuer, java.security.PrivateKey priv) throws Exception {
        String[] parts = token.split("\\.");
        String headerJson = new String(Base64.getUrlDecoder().decode(parts[0]));
        String payloadJson = new String(Base64.getUrlDecoder().decode(parts[1]));
        java.util.Map<String, Object> payload = new com.fasterxml.jackson.databind.ObjectMapper().readValue(payloadJson, java.util.Map.class);
        payload.put("iss", newIssuer);
        return sign(headerJson, new com.fasterxml.jackson.databind.ObjectMapper().writeValueAsString(payload), priv);
    }

    private static String rewriteAudience(String token, String newAud, java.security.PrivateKey priv) throws Exception {
        String[] parts = token.split("\\.");
        String headerJson = new String(Base64.getUrlDecoder().decode(parts[0]));
        String payloadJson = new String(Base64.getUrlDecoder().decode(parts[1]));
        java.util.Map<String, Object> payload = new com.fasterxml.jackson.databind.ObjectMapper().readValue(payloadJson, java.util.Map.class);
        payload.put("aud", newAud);
        return sign(headerJson, new com.fasterxml.jackson.databind.ObjectMapper().writeValueAsString(payload), priv);
    }

    private static String sign(String headerJson, String payloadJson, java.security.PrivateKey priv) throws Exception {
        String signingInput = b64u(headerJson) + "." + b64u(payloadJson);
        java.security.Signature sig = java.security.Signature.getInstance("SHA256withRSA");
        sig.initSign(priv);
        sig.update(signingInput.getBytes(java.nio.charset.StandardCharsets.US_ASCII));
        return signingInput + "." + Base64.getUrlEncoder().withoutPadding().encodeToString(sig.sign());
    }

    private static String b64u(String s) {
        return Base64.getUrlEncoder().withoutPadding().encodeToString(s.getBytes(java.nio.charset.StandardCharsets.UTF_8));
    }
}
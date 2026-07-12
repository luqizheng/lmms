package com.coder.pm.config;

import io.jsonwebtoken.Jwts;
import io.jsonwebtoken.security.Keys;
import org.springframework.boot.context.properties.ConfigurationProperties;
import org.springframework.context.annotation.Bean;
import org.springframework.context.annotation.Configuration;

import javax.crypto.SecretKey;
import java.nio.charset.StandardCharsets;

/**
 * JWT signing config: HS256, shared secret loaded from {@code pm.jwt.*}.
 *
 * <p>apps/pm v0.1.0 ships with self-issued JWT (HMAC). Future versions may
 * switch to RS256 + shared JWKS with iam-service.
 */
@Configuration
@ConfigurationProperties(prefix = "pm.jwt")
public class JwtConfig {

    /** Shared secret. Must be >= 32 bytes for HS256. */
    private String secret = "dev-secret-change-me-min-256-bits-long-enough-please";
    private long expirationMinutes = 60;
    private String issuer = "pm-platform";

    @Bean
    public SecretKey jwtSecretKey() {
        byte[] keyBytes = secret.getBytes(StandardCharsets.UTF_8);
        if (keyBytes.length < 32) {
            throw new IllegalStateException(
                    "pm.jwt.secret must be >= 32 bytes for HS256 (got " + keyBytes.length + " bytes). " +
                            "Set PM_JWT_SECRET env var.");
        }
        return Keys.hmacShaKeyFor(keyBytes);
    }

    @Bean
    public io.jsonwebtoken.JwtParser jwtParser(SecretKey jwtSecretKey) {
        return Jwts.parser()
                .verifyWith(jwtSecretKey)
                .requireIssuer(issuer)
                .build();
    }

    public String getSecret() { return secret; }
    public void setSecret(String secret) { this.secret = secret; }

    public long getExpirationMinutes() { return expirationMinutes; }
    public void setExpirationMinutes(long expirationMinutes) { this.expirationMinutes = expirationMinutes; }

    public String getIssuer() { return issuer; }
    public void setIssuer(String issuer) { this.issuer = issuer; }
}

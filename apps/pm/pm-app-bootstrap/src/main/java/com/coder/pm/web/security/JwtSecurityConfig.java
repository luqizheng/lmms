package com.coder.pm.web.security;

import org.springframework.beans.factory.annotation.Value;
import org.springframework.context.annotation.Bean;
import org.springframework.context.annotation.Configuration;

/**
 * Wires up the two {@link JwtVerifier} delegates explicitly so the
 * {@code @Value}-annotated constructors can be used without depending on
 * Spring's component scan. {@link JwtVerifierRouter} is the single
 * {@code @Component} {@link JwtVerifier} bean and forwards to the active one
 * based on {@code pm.jwt.mode}.
 */
@Configuration
public class JwtSecurityConfig {

    @Bean
    public JwksJwtVerifier jwksJwtVerifier(
            @Value("${pm.jwt.jwks-url:http://localhost:3000/v1/tokens/.well-known/jwks.json}") String jwksUrl,
            @Value("${pm.jwt.issuer:superpowers-token-service}") String issuer,
            @Value("${pm.jwt.audience:coder-micro-services}") String audience,
            @Value("${pm.jwt.required-scope:access}") String requiredScope,
            @Value("${pm.jwt.cache-ttl-minutes:10}") long cacheTtlMinutes) {
        return new JwksJwtVerifier(jwksUrl, issuer, audience, requiredScope, cacheTtlMinutes);
    }

    @Bean
    public Hs256JwtVerifier hs256JwtVerifier(
            @Value("${pm.jwt.hmac-secret:}") String hmacSecret,
            @Value("${pm.jwt.issuer:superpowers-token-service}") String issuer,
            @Value("${pm.jwt.required-scope:access}") String requiredScope) {
        return new Hs256JwtVerifier(hmacSecret, issuer, requiredScope);
    }
}
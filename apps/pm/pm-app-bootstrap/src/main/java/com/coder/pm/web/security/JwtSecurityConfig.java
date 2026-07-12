package com.coder.pm.web.security;

import org.springframework.beans.factory.annotation.Value;
import org.springframework.context.annotation.Bean;
import org.springframework.context.annotation.Configuration;
import org.springframework.context.annotation.Profile;

/**
 * Wires up the JWT verification pipeline.
 *
 * <p>Two parallel configurations, gated by Spring profile:
 * <ul>
 *   <li><b>prod-ish</b> ({@code !dev}): the production setup. {@link JwksJwtVerifier}
 *       pulls keys from the iam-service JWKS endpoint and validates RS256
 *       signatures. {@link Hs256JwtVerifier} is wired as a fallback in case
 *       someone sets {@code PM_JWT_MODE=hs256} for offline work. The
 *       {@link JwtVerifierRouter} picks between them based on
 *       {@code pm.jwt.mode}. Selected by {@code @Profile("!dev")}.</li>
 *   <li><b>dev</b> ({@code dev}): {@link NoOpJwtVerifier} returns a fixed
 *       {@code dev} {@link Principal} for every request, skipping all crypto
 *       and network checks. Lets a developer curl any endpoint without first
 *       logging in via iam-service. Selected by {@code @Profile("dev")}.</li>
 * </ul>
 *
 * <p>Both halves produce exactly one {@link JwtVerifier} bean named
 * {@link JwtVerifierRouter#BEAN_NAME} so {@link com.coder.pm.web.JwtAuthFilter}
 * can autowire without qualifiers.
 */
@Configuration
public class JwtSecurityConfig {

    /**
     * Production configuration. Default-validates everything via JWKS.
     */
    @Configuration
    @Profile("!dev")
    static class Production {

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

        @Bean(JwtVerifierRouter.BEAN_NAME)
        public JwtVerifierRouter prodJwtVerifierRouter(
                JwksJwtVerifier jwks,
                Hs256JwtVerifier hs256,
                @Value("${pm.jwt.mode:jwks}") String modeRaw) {
            return new JwtVerifierRouter(jwks, hs256, modeRaw);
        }
    }

    /**
     * Dev configuration. Returns a {@link NoOpJwtVerifier} for every token,
     * which means curl / frontend can hit any protected endpoint without
     * first logging in via iam-service. Never enable this profile in
     * production — the {@code @Profile("dev")} guard makes that difficult to
     * do accidentally.
     */
    @Configuration
    @Profile("dev")
    static class Development {

        @Bean
        public JwksJwtVerifier devJwksJwtVerifier() {
            // Wired so that JwtVerifierRouter can be constructed if needed
            // (e.g. someone flips PM_JWT_MODE=jwks while in dev). The router
            // itself ignores this bean because pm.jwt.mode=noop in dev.
            return new JwksJwtVerifier(
                    "http://localhost:0/.well-known/jwks.json",
                    "superpowers-token-service",
                    "coder-micro-services",
                    "access",
                    10);
        }

        @Bean
        public Hs256JwtVerifier devHs256JwtVerifier() {
            return new Hs256JwtVerifier("", "superpowers-token-service", "access");
        }

        @Bean(JwtVerifierRouter.BEAN_NAME)
        public JwtVerifierRouter devJwtVerifierRouter(
                JwksJwtVerifier jwks,
                Hs256JwtVerifier hs256,
                @Value("${pm.jwt.mode:noop}") String modeRaw) {
            return new JwtVerifierRouter(jwks, hs256, modeRaw);
        }
    }
}
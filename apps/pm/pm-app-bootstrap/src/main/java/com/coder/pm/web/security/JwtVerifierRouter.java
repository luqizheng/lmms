package com.coder.pm.web.security;

import org.springframework.beans.factory.annotation.Value;

/**
 * Selects which {@link JwtVerifier} implementation is active based on the
 * {@code pm.jwt.mode} configuration property. Spring picks exactly one
 * delegate (either {@link JwksJwtVerifier}, {@link Hs256JwtVerifier}, or
 * {@link NoOpJwtVerifier}) and the router just forwards to it.
 *
 * <p>Default mode is {@code jwks} so that a misconfigured production
 * environment degrades to "talks to iam-service / token-service" rather than
 * silently accepting tokens. {@code hs256} is opt-in for offline work.
 * {@code noop} is the dev-profile short-circuit that returns a synthetic
 * Principal for every request so a developer can curl any protected endpoint
 * without first logging in via iam-service.
 *
 * <p>NOT annotated with {@code @Component} on purpose — Spring wires this
 * bean only via {@link JwtSecurityConfig}, which gates the prod and dev
 * sub-configs by Spring profile. Annotation here would defeat the gate.
 */
public class JwtVerifierRouter implements JwtVerifier {

    /** Stable bean name for explicit injection by tests and other components. */
    public static final String BEAN_NAME = "jwtVerifierRouter";

    public enum Mode { JWKS, HS256, NOOP }

    private final Mode mode;
    private final JwksJwtVerifier jwks;
    private final Hs256JwtVerifier hs256;
    private final NoOpJwtVerifier noop;

    public JwtVerifierRouter(
            JwksJwtVerifier jwks,
            Hs256JwtVerifier hs256,
            @Value("${pm.jwt.mode:jwks}") String modeRaw) {
        this.jwks = jwks;
        this.hs256 = hs256;
        this.noop = new NoOpJwtVerifier();
        Mode parsed;
        try {
            parsed = Mode.valueOf(modeRaw.trim().toUpperCase());
        } catch (IllegalArgumentException ex) {
            parsed = Mode.JWKS;
        }
        this.mode = parsed;
    }

    /** Test / dev convenience overload that locks mode to jwks. */
    public JwtVerifierRouter(JwksJwtVerifier jwks, Hs256JwtVerifier hs256) {
        this(jwks, hs256, "jwks");
    }

    public Mode mode() {
        return mode;
    }

    @Override
    public Principal verify(String token) {
        return switch (mode) {
            case HS256 -> hs256.verify(token);
            case NOOP -> noop.verify(token);
            case JWKS -> jwks.verify(token);
        };
    }
}
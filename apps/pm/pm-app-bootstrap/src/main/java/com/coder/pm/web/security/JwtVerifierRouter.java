package com.coder.pm.web.security;

import org.springframework.beans.factory.annotation.Value;
import org.springframework.stereotype.Component;

/**
 * Selects which {@link JwtVerifier} implementation is active based on the
 * {@code pm.jwt.mode} configuration property. Spring picks exactly one
 * delegate (either {@link JwksJwtVerifier} or {@link Hs256JwtVerifier}) and
 * the router just forwards to it.
 *
 * <p>Default mode is {@code jwks} so that a misconfigured environment
 * degrades to "talks to iam-service / token-service" rather than silently
 * accepting an HS256 secret nobody set. HS256 is opt-in via
 * {@code PM_JWT_MODE=hs256} for offline / dev work.
 *
 * <p>This class is the only {@code @Component} {@link JwtVerifier} bean; the
 * two delegates are created internally via Spring's factory lookup so the
 * autowiring of {@link JwtVerifier} into {@link JwtAuthFilter} is unambiguous.
 */
@Component(JwtVerifierRouter.BEAN_NAME)
public class JwtVerifierRouter implements JwtVerifier {

    /** Stable bean name for explicit injection by tests and other components. */
    public static final String BEAN_NAME = "jwtVerifierRouter";

    public enum Mode { JWKS, HS256 }

    private final Mode mode;
    private final JwksJwtVerifier jwks;
    private final Hs256JwtVerifier hs256;

    /**
     * Look up the delegates by their fully-qualified class so Spring can
     * construct them via their {@code @Value}-annotated constructors without
     * needing {@code @Component} on the delegates themselves.
     */
    public JwtVerifierRouter(
            JwksJwtVerifier jwks,
            Hs256JwtVerifier hs256,
            @Value("${pm.jwt.mode:jwks}") String modeRaw) {
        Mode parsed;
        try {
            parsed = Mode.valueOf(modeRaw.trim().toUpperCase());
        } catch (IllegalArgumentException ex) {
            parsed = Mode.JWKS;
        }
        this.mode = parsed;
        this.jwks = jwks;
        this.hs256 = hs256;
    }

    public Mode mode() {
        return mode;
    }

    @Override
    public Principal verify(String token) {
        return mode == Mode.HS256 ? hs256.verify(token) : jwks.verify(token);
    }
}
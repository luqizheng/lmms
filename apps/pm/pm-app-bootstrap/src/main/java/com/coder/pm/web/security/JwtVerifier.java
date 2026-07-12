package com.coder.pm.web.security;

/**
 * Verifies a bearer token and returns the {@link Principal} it represents, or
 * {@link Principal#ANONYMOUS} when verification fails / no token is provided.
 *
 * <p>Implementations:
 * <ul>
 *   <li>{@link JwksJwtVerifier} — RS256, pulls keys from iam-service's JWKS
 *       endpoint. Production / shared-tenant mode.</li>
 *   <li>{@link Hs256JwtVerifier} — HS256 with a shared secret. Local-dev mode
 *       for offline work; <strong>not</strong> for production.</li>
 * </ul>
 *
 * <p>Implementations MUST be thread-safe and MUST NOT throw on bad input —
 * return {@link Principal#ANONYMOUS} instead so {@link JwtAuthFilter} can
 * cleanly let the security chain reject as 401.
 */
public interface JwtVerifier {

    /**
     * @param token raw bearer token (without the {@code Bearer } prefix)
     * @return verified principal, or {@link Principal#ANONYMOUS} on any error
     */
    Principal verify(String token);
}
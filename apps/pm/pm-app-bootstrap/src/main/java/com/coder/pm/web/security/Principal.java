package com.coder.pm.web.security;

import java.util.List;

/**
 * Immutable identity extracted from a verified JWT.
 *
 * <p>Maps the user-jwt claims issued by iam-service (services/iam-service +
 * services/token-service::TokenIssuer::issue_user_jwt) into a Spring-friendly
 * representation. Used as the {@code principal} field on the
 * {@link org.springframework.security.core.Authentication} created by
 * {@link JwtAuthFilter}.
 *
 * @param userId      numeric id derived from {@code sub} (UUID hex → first 8 bytes → long);
 *                    null if the sub is not parseable
 * @param username    {@code user_code} claim (human-readable login name)
 * @param tenantId    {@code tid} claim (long); null when missing
 * @param roles       {@code roles} claim (string[])
 * @param permissions {@code permissions} claim (string[])
 * @param scope       {@code scope} claim — typical values: {@code access}, {@code refresh},
 *                    {@code resource}. Filters which token kinds this verifier should accept.
 */
public record Principal(
        Long userId,
        String username,
        Long tenantId,
        List<String> roles,
        List<String> permissions,
        String scope
) {
    /** Empty principal — used when a JWT is missing or unverified. */
    public static final Principal ANONYMOUS =
            new Principal(null, null, null, List.of(), List.of(), null);

    public boolean isAnonymous() {
        return userId == null && username == null;
    }

    public boolean hasRole(String role) {
        return roles != null && roles.contains(role);
    }

    public boolean hasPermission(String permission) {
        return permissions != null && permissions.contains(permission);
    }
}
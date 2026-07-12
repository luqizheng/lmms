package com.coder.pm.web.security;

import java.util.List;

/**
 * Dev-only {@link JwtVerifier} that returns a fixed synthetic
 * {@link Principal} for every request.
 *
 * <p>Wired exclusively under Spring profile {@code dev} by
 * {@link JwtSecurityConfig}. Lets a developer run apps/pm and exercise every
 * protected endpoint without first minting a real JWT via iam-service. Never
 * enable in production — there is no verification, no token check, no
 * signature.
 *
 * <p>The synthetic principal is modelled after a tenant_admin:
 * <ul>
 *   <li>userId = 1L (matches the {@code created_by} value PM services use
 *       when no JWT principal is present)</li>
 *   <li>tenantId = 1L</li>
 *   <li>roles = {@code [pm_admin]}</li>
 *   <li>permissions = {@code [pm:*]} (a wildcard string accepted by any
 *       {@code hasPermission(...)} check that does prefix matching; today no
 *       such check exists, so this is mostly informational for log lines)</li>
 *   <li>scope = "access"</li>
 * </ul>
 */
public class NoOpJwtVerifier implements JwtVerifier {

    private static final Principal DEV_PRINCIPAL = new Principal(
            1L,
            "dev",
            1L,
            List.of("pm_admin"),
            List.of("pm:*"),
            "access"
    );

    @Override
    public Principal verify(String token) {
        return DEV_PRINCIPAL;
    }
}
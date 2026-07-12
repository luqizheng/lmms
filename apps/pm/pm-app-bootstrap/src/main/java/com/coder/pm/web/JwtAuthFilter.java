package com.coder.pm.web;

import com.coder.pm.web.security.JwtVerifier;
import com.coder.pm.web.security.JwtVerifierRouter;
import com.coder.pm.web.security.Principal;
import jakarta.servlet.FilterChain;
import jakarta.servlet.ServletException;
import jakarta.servlet.http.HttpServletRequest;
import jakarta.servlet.http.HttpServletResponse;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.springframework.beans.factory.annotation.Qualifier;
import org.springframework.security.authentication.AbstractAuthenticationToken;
import org.springframework.security.core.GrantedAuthority;
import org.springframework.security.core.authority.SimpleGrantedAuthority;
import org.springframework.security.core.context.SecurityContextHolder;
import org.springframework.security.web.authentication.WebAuthenticationDetailsSource;
import org.springframework.stereotype.Component;
import org.springframework.web.filter.OncePerRequestFilter;

import java.io.IOException;
import java.util.ArrayList;
import java.util.Collection;
import java.util.List;

/**
 * Bearer-JWT authentication filter.
 *
 * <p>Reads {@code Authorization: Bearer <jwt>}, hands the raw token to the
 * configured {@link JwtVerifier} (RS256 via JWKS in prod / HS256 in dev),
 * and installs the resulting {@link Principal} on the Spring SecurityContext
 * so downstream handlers can inject it via
 * {@code @AuthenticationPrincipal Principal}.
 *
 * <p>Failure modes (missing header, malformed token, bad signature, expired
 * token, wrong issuer/audience/scope) all collapse to "no authentication set"
 * — the security chain will then reject the request as 401, matching the
 * platform's contract (see {@code docs/ai-context.md} pitfall 19).
 */
@Component
public class JwtAuthFilter extends OncePerRequestFilter {

    private static final Logger log = LoggerFactory.getLogger(JwtAuthFilter.class);
    private static final String HEADER = "Authorization";
    private static final String PREFIX = "Bearer ";

    private final JwtVerifier verifier;

    public JwtAuthFilter(@Qualifier(JwtVerifierRouter.BEAN_NAME) JwtVerifier verifier) {
        this.verifier = verifier;
    }

    @Override
    protected void doFilterInternal(HttpServletRequest request,
                                    HttpServletResponse response,
                                    FilterChain filterChain) throws ServletException, IOException {
        log.info("JwtAuthFilter entered for {} {}", request.getMethod(), request.getRequestURI());
        String token = extractBearer(request.getHeader(HEADER));
        // Always call verify — even when the token is missing. Production
        // verifiers return Principal.ANONYMOUS for null input, which leaves
        // the SecurityContext empty (and Spring Security returns 401/403
        // for protected endpoints, as it should). Dev-profile verifiers
        // (NoOpJwtVerifier) return a synthetic dev principal regardless of
        // input, which lets a developer curl protected endpoints without a
        // token. Both branches honour the same SecurityContext contract.
        Principal principal = verifier.verify(token);
        if (!principal.isAnonymous()) {
            AbstractAuthenticationToken auth = new AuthToken(principal, authorities(principal));
            auth.setDetails(new WebAuthenticationDetailsSource().buildDetails(request));
            SecurityContextHolder.getContext().setAuthentication(auth);
        } else {
            log.debug("JWT rejected; clearing SecurityContext");
            SecurityContextHolder.clearContext();
        }
        filterChain.doFilter(request, response);
    }

    private static String extractBearer(String header) {
        if (header == null || !header.startsWith(PREFIX)) return null;
        return header.substring(PREFIX.length()).trim();
    }

    /** Roles are exposed as {@code ROLE_<UPPER>} authorities; permissions keep their canonical form. */
    private static Collection<? extends GrantedAuthority> authorities(Principal p) {
        List<GrantedAuthority> out = new ArrayList<>();
        if (p.roles() != null) {
            for (String r : p.roles()) {
                if (r != null && !r.isBlank()) out.add(new SimpleGrantedAuthority("ROLE_" + r.toUpperCase()));
            }
        }
        if (p.permissions() != null) {
            for (String perm : p.permissions()) {
                if (perm != null && !perm.isBlank()) out.add(new SimpleGrantedAuthority(perm));
            }
        }
        return out;
    }

    /**
     * Authentication that carries the rich {@link Principal} instead of just a
     * username string. The principal exposes userId, tenantId, roles, and
     * permissions so controllers don't have to re-parse JWT claims.
     */
    private static final class AuthToken extends AbstractAuthenticationToken {
        private final Principal principal;

        AuthToken(Principal principal, Collection<? extends GrantedAuthority> authorities) {
            super(authorities);
            this.principal = principal;
            super.setAuthenticated(true);
        }

        @Override public Object getCredentials() { return ""; }
        @Override public Principal getPrincipal() { return principal; }
        @Override public String getName() { return principal.username() != null ? principal.username() : "anonymous"; }
    }
}
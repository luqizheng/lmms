package com.coder.pm.infra.security;

import com.coder.pm.domain.security.Principal;
import org.springframework.security.core.Authentication;
import org.springframework.security.core.context.SecurityContextHolder;

/**
 * Reads the current caller out of Spring's {@link SecurityContextHolder} in a
 * way that understands the {@link Principal} record that
 * {@code com.coder.pm.web.JwtAuthFilter} installs.
 *
 * <p>Used by every domain service that needs an actor id (for audit reports,
 * for filling in {@code created_by} columns) or an actor display name.
 *
 * <p>Why this exists: the same pattern used to be copy-pasted across
 * {@code ProjectService / RequirementService / UserStoryService / ...}. Each
 * copy had a slightly different fallback (some used
 * {@code Long.valueOf(String.valueOf(principal))} which threw on the
 * {@code Principal} record's toString, others just dumped the principal to
 * the audit log). Centralising it eliminates the divergence and gives one
 * place to evolve when the SecurityContext contract changes again.
 */
public final class CurrentActor {

    private CurrentActor() {}

    /** Read the principal object off the SecurityContext, or {@code null}. */
    public static Principal principalOrNull() {
        Authentication auth = SecurityContextHolder.getContext().getAuthentication();
        if (auth == null) return null;
        Object p = auth.getPrincipal();
        if (p instanceof Principal pr) return pr;
        return null;
    }

    /**
     * Numeric user id of the current caller, suitable for {@code created_by}
     * columns. Falls back to {@code 0} when no JWT is present (dev / system
     * flow). Returns {@code null} only if the principal exists but exposes no
     * id — caller can decide whether that's a hard error.
     */
    public static Long actorIdOrZero() {
        Principal p = principalOrNull();
        return p != null && p.userId() != null ? p.userId() : 0L;
    }

    /** Same as {@link #actorIdOrZero()} but {@code null} when no JWT. */
    public static Long actorIdOrNull() {
        Principal p = principalOrNull();
        return p != null ? p.userId() : null;
    }

    /**
     * Display name for audit logs / response bodies. {@code "system"} when
     * no JWT is present (dev profile is OK — auth is bypassed but logs should
     * still be meaningful); the {@link Principal#username()} otherwise.
     */
    public static String displayName() {
        Principal p = principalOrNull();
        return p != null && p.username() != null ? p.username() : "system";
    }
}
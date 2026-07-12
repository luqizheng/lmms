package com.coder.pm.infra.security;

import com.coder.pm.domain.security.Principal;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Test;
import org.springframework.security.authentication.UsernamePasswordAuthenticationToken;
import org.springframework.security.core.Authentication;
import org.springframework.security.core.authority.SimpleGrantedAuthority;
import org.springframework.security.core.context.SecurityContextHolder;

import java.util.List;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertNull;

/**
 * Unit tests for {@link CurrentActor}. Uses a raw {@link SecurityContextHolder}
 * stand-in (the filter that would normally install it is exercised in the
 * Phase B end-to-end smoke).
 */
class CurrentActorTest {

    @AfterEach
    void clear() {
        SecurityContextHolder.clearContext();
    }

    @Test
    void returnsSystemWhenNoContext() {
        SecurityContextHolder.clearContext();
        assertNull(CurrentActor.actorIdOrNull());
        assertEquals("system", CurrentActor.displayName());
    }

    @Test
    void returnsSystemWhenContextIsAnonymous() {
        Authentication anon = new UsernamePasswordAuthenticationToken(
                "anonymousUser", null, List.of());
        SecurityContextHolder.getContext().setAuthentication(anon);
        assertNull(CurrentActor.actorIdOrNull());
        assertEquals("system", CurrentActor.displayName());
    }

    @Test
    void readsFieldsOutOfPrincipalRecord() {
        Principal p = new Principal(42L, "alice", 7L,
                List.of("pm_admin"), List.of("pm:project_create"), "access");
        SecurityContextHolder.getContext().setAuthentication(
                new UsernamePasswordAuthenticationToken(p, null,
                        List.of(new SimpleGrantedAuthority("ROLE_PM_ADMIN"))));
        assertEquals(42L, CurrentActor.actorIdOrNull());
        assertEquals(42L, CurrentActor.actorIdOrZero());
        assertEquals("alice", CurrentActor.displayName());
        assertEquals(p, CurrentActor.principalOrNull());
    }

    @Test
    void fallsBackToZeroWhenPrincipalHasNoUserId() {
        Principal anon = Principal.ANONYMOUS;
        SecurityContextHolder.getContext().setAuthentication(
                new UsernamePasswordAuthenticationToken(anon, null, List.of()));
        assertNull(CurrentActor.actorIdOrNull());
        assertEquals(0L, CurrentActor.actorIdOrZero());
        assertEquals("system", CurrentActor.displayName());
    }
}
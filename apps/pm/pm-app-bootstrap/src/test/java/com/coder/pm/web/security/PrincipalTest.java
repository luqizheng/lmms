package com.coder.pm.web.security;

import com.coder.pm.domain.security.Principal;
import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

class PrincipalTest {

    @Test
    void anonymousDefaultsToEmpty() {
        Principal p = Principal.ANONYMOUS;
        assertTrue(p.isAnonymous());
        assertFalse(p.hasRole("anything"));
        assertFalse(p.hasPermission("anything"));
    }

    @Test
    void hasRoleAndPermissionRequireExactMatch() {
        Principal p = new Principal(1L, "alice", 7L,
                java.util.List.of("pm_admin", "viewer"),
                java.util.List.of("pm_project_create"), "access");
        assertFalse(p.isAnonymous());
        assertTrue(p.hasRole("pm_admin"));
        assertTrue(p.hasRole("viewer"));
        assertFalse(p.hasRole("missing"));
        assertTrue(p.hasPermission("pm_project_create"));
        assertFalse(p.hasPermission("pm_project_delete"));
    }

    @Test
    void roleAndPermissionAreIndependent() {
        Principal p = new Principal(1L, "alice", null, java.util.List.of(), java.util.List.of(), null);
        assertFalse(p.hasRole("x"));
        assertFalse(p.hasPermission("y"));
        assertEquals(1L, p.userId());
    }
}
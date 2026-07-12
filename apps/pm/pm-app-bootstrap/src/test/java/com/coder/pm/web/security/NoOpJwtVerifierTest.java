package com.coder.pm.web.security;

import com.coder.pm.domain.security.Principal;
import org.junit.jupiter.api.Test;

import java.util.List;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertSame;
import static org.junit.jupiter.api.Assertions.assertTrue;

class NoOpJwtVerifierTest {

    @Test
    void returnsSamePrincipalForAnyInput() {
        NoOpJwtVerifier v = new NoOpJwtVerifier();
        Principal a = v.verify(null);
        Principal b = v.verify("");
        Principal c = v.verify("garbage");
        Principal d = v.verify("eyJhbGciOiJSUzI1NiJ9.aaa.bbb");

        assertFalse(a.isAnonymous());
        assertEquals(1L, a.userId());
        assertEquals("dev", a.username());
        assertEquals(1L, a.tenantId());
        assertEquals(List.of("pm_admin"), a.roles());
        assertEquals(List.of("pm:*"), a.permissions());
        assertEquals("access", a.scope());

        assertSame(a, b, "NoOpJwtVerifier must return the same principal for every call");
        assertSame(a, c);
        assertSame(a, d);
    }

    @Test
    void syntheticPrincipalHasAdminRole() {
        Principal p = new NoOpJwtVerifier().verify("anything");
        assertTrue(p.hasRole("pm_admin"));
        assertTrue(p.hasPermission("pm:*"));
        assertFalse(p.hasRole("missing"));
        assertFalse(p.hasPermission("iam:user_read"));
    }
}
package com.coder.pm.infra.service;

import com.coder.pm.domain.security.Principal;
import com.coder.pm.infra.persistence.entity.KbSpace;
import com.coder.pm.infra.persistence.mapper.KbSpaceMapper;
import com.coder.pm.infra.sdk.AuditEventReporter;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.extension.ExtendWith;
import org.mockito.Mock;
import org.mockito.junit.jupiter.MockitoExtension;
import org.springframework.security.authentication.UsernamePasswordAuthenticationToken;
import org.springframework.security.core.context.SecurityContextHolder;

import java.util.List;

import static org.junit.jupiter.api.Assertions.*;
import static org.mockito.ArgumentMatchers.any;
import static org.mockito.Mockito.*;

@ExtendWith(MockitoExtension.class)
class KbSpaceServiceTest {

    @Mock private AuditEventReporter audit;
    @Mock private KbSpaceMapper mapper;

    private KbSpaceService service;

    @BeforeEach
    void setup() {
        service = spy(new KbSpaceService(audit));
        try {
            var field = com.baomidou.mybatisplus.extension.service.impl.ServiceImpl.class
                    .getDeclaredField("baseMapper");
            field.setAccessible(true);
            field.set(service, mapper);
        } catch (Exception ignored) {}
    }

    @AfterEach
    void clearSecurity() { SecurityContextHolder.clearContext(); }

    private void setPrincipal(Long userId, String username) {
        Principal p = new Principal(userId, username, 1L, List.of("pm_admin"), List.of("pm:*"), "access");
        SecurityContextHolder.getContext().setAuthentication(
                new UsernamePasswordAuthenticationToken(p, null, List.of()));
    }

    private KbSpace validDraft() {
        KbSpace s = new KbSpace();
        s.setName("Test Space");
        s.setProjectId(1L);
        return s;
    }

    @Test
    void create_throwsOnInvalidVisibility() {
        KbSpace s = validDraft();
        s.setVisibility("secret");
        assertThrows(IllegalArgumentException.class, () -> service.create(s));
        verify(service, never()).save(any());
    }

    @Test
    void create_setsDefaultVisibilityProject() {
        setPrincipal(1L, "dev");
        KbSpace s = validDraft();
        doReturn(true).when(service).save(any(KbSpace.class));
        KbSpace result = service.create(s);
        assertEquals("project", result.getVisibility());
    }

    @Test
    void create_fillsCreatedByFromPrincipal() {
        setPrincipal(55L, "dave");
        KbSpace s = validDraft();
        s.setVisibility("private");
        doReturn(true).when(service).save(any(KbSpace.class));
        KbSpace result = service.create(s);
        assertEquals(55L, result.getCreatedBy());
        verify(audit).report(eq("dave"), eq("KB_SPACE_CREATED"), anyString(), anyString(), any());
    }

    @Test
    void create_acceptsAllValidVisibilityValues() {
        setPrincipal(1L, "dev");
        for (String v : List.of("private", "project", "open")) {
            KbSpace s = validDraft();
            s.setName("Space-" + v);
            s.setVisibility(v);
            doReturn(true).when(service).save(any(KbSpace.class));
            KbSpace result = service.create(s);
            assertEquals(v, result.getVisibility());
        }
    }
}
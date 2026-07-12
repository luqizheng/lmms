package com.coder.pm.infra.service;

import com.coder.pm.domain.security.Principal;
import com.coder.pm.infra.persistence.entity.Sprint;
import com.coder.pm.infra.persistence.mapper.SprintMapper;
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
import static org.mockito.ArgumentMatchers.*;
import static org.mockito.Mockito.*;

@ExtendWith(MockitoExtension.class)
class SprintServiceTest {

    @Mock private AuditEventReporter audit;
    @Mock private ProjectService projectService;
    @Mock private SprintMapper mapper;

    private SprintService service;

    @BeforeEach
    void setup() {
        service = spy(new SprintService(audit, projectService));
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

    private Sprint validDraft() {
        Sprint s = new Sprint();
        s.setProjectId(1L);
        s.setName("Sprint 1");
        return s;
    }

    @Test
    void create_validatesProjectExists() {
        Sprint s = validDraft();
        doThrow(new com.coder.pm.web.exception.NotFoundException("project 99 not found"))
                .when(projectService).findActiveOrThrow(99L);
        s.setProjectId(99L);
        assertThrows(com.coder.pm.web.exception.NotFoundException.class, () -> service.create(s));
        verify(service, never()).save(any());
    }

    @Test
    void create_setsDefaultsAndFillsCreatedBy() {
        setPrincipal(42L, "alice");
        Sprint s = validDraft();
        doAnswer(inv -> { ((Sprint) inv.getArgument(0)).setId(7L); return true; })
                .when(service).save(any(Sprint.class));
        Sprint result = service.create(s);
        assertEquals("planning", result.getStatus());
        assertEquals(42L, result.getCreatedBy());
        verify(audit).report(eq("alice"), eq("PM_SPRINT_CREATED"), anyString(), anyString(), any());
    }
}
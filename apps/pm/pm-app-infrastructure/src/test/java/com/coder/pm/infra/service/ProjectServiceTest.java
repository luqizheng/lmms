package com.coder.pm.infra.service;

import com.baomidou.mybatisplus.core.conditions.query.LambdaQueryWrapper;
import com.baomidou.mybatisplus.extension.conditions.query.LambdaQueryChainWrapper;
import com.coder.pm.domain.security.Principal;
import com.coder.pm.infra.persistence.entity.Project;
import com.coder.pm.infra.persistence.mapper.ProjectMapper;
import com.coder.pm.infra.sdk.AuditEventReporter;
import com.coder.pm.web.exception.ConflictException;
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

/**
 * Unit tests for {@link ProjectService}. Uses Mockito spies to stub the
 * MyBatis-Plus ServiceImpl DB methods (save, lambdaQuery) while exercising
 * the real business logic (duplicate-code check, default status, createdBy
 * injection from CurrentActor, audit reporting).
 */
@ExtendWith(MockitoExtension.class)
class ProjectServiceTest {

    @Mock private AuditEventReporter audit;
    @Mock private ProjectMapper mapper;
    @Mock private LambdaQueryChainWrapper<Project> queryChain;

    private ProjectService service;

    @BeforeEach
    void setup() {
        service = spy(new ProjectService(audit));
        // Inject the mock mapper so ServiceImpl internal calls don't NPE.
        // Reflection because baseMapper is a protected field on ServiceImpl.
        try {
            var field = com.baomidou.mybatisplus.extension.service.impl.ServiceImpl.class
                    .getDeclaredField("baseMapper");
            field.setAccessible(true);
            field.set(service, mapper);
        } catch (Exception ignored) {}
    }

    @AfterEach
    void clearSecurity() {
        SecurityContextHolder.clearContext();
    }

    private void setPrincipal(Long userId, String username) {
        Principal p = new Principal(userId, username, 1L,
                List.of("pm_admin"), List.of("pm:*"), "access");
        SecurityContextHolder.getContext().setAuthentication(
                new UsernamePasswordAuthenticationToken(p, null, List.of()));
    }

    @Test
    void create_throwsConflictWhenCodeAlreadyExists() {
        Project draft = new Project();
        draft.setCode("DEMO");
        draft.setName("Test");

        doReturn(queryChain).when(service).lambdaQuery();
        doReturn(queryChain).when(queryChain).eq(any(), any());
        doReturn(1L).when(queryChain).count();

        assertThrows(ConflictException.class, () -> service.create(draft));
        verify(service, never()).save(any());
    }

    @Test
    void create_setsDefaultStatusActiveAndFillsCreatedByFromPrincipal() {
        setPrincipal(42L, "alice");

        Project draft = new Project();
        draft.setCode("NEW");
        draft.setName("New Project");
        draft.setOwnerId(1L);

        doReturn(queryChain).when(service).lambdaQuery();
        doReturn(queryChain).when(queryChain).eq(any(), any());
        doReturn(0L).when(queryChain).count();
        doReturn(true).when(service).save(any(Project.class));

        Project result = service.create(draft);

        assertEquals("active", result.getStatus());
        assertEquals(42L, result.getCreatedBy());
        verify(audit).report(eq("alice"), eq("PM_PROJECT_CREATED"), eq("pm_project"), anyString(), any());
    }

    @Test
    void create_doesNotOverrideExplicitlySetCreatedBy() {
        setPrincipal(42L, "alice");

        Project draft = new Project();
        draft.setCode("NEW2");
        draft.setName("New Project 2");
        draft.setOwnerId(1L);
        draft.setCreatedBy(99L);

        doReturn(queryChain).when(service).lambdaQuery();
        doReturn(queryChain).when(queryChain).eq(any(), any());
        doReturn(0L).when(queryChain).count();
        doReturn(true).when(service).save(any(Project.class));

        Project result = service.create(draft);

        assertEquals(99L, result.getCreatedBy());
    }

    @Test
    void create_usesSystemAsActorWhenNoPrincipal() {
        Project draft = new Project();
        draft.setCode("NEW3");
        draft.setName("System Project");
        draft.setOwnerId(1L);

        doReturn(queryChain).when(service).lambdaQuery();
        doReturn(queryChain).when(queryChain).eq(any(), any());
        doReturn(0L).when(queryChain).count();
        doReturn(true).when(service).save(any(Project.class));

        service.create(draft);

        verify(audit).report(eq("system"), eq("PM_PROJECT_CREATED"), eq("pm_project"), anyString(), any());
    }
}
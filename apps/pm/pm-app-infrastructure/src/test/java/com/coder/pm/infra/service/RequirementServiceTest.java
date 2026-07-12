package com.coder.pm.infra.service;

import com.coder.pm.domain.security.Principal;
import com.coder.pm.infra.persistence.entity.Requirement;
import com.coder.pm.infra.persistence.mapper.RequirementMapper;
import com.coder.pm.infra.sdk.AuditEventReporter;
import com.coder.pm.web.exception.NotFoundException;
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
import static org.mockito.ArgumentMatchers.anyString;
import static org.mockito.Mockito.*;

/**
 * Unit tests for {@link RequirementService}. Focuses on enum validation
 * (which runs before any DB call) and CurrentActor integration.
 */
@ExtendWith(MockitoExtension.class)
class RequirementServiceTest {

    @Mock private AuditEventReporter audit;
    @Mock private RequirementMapper mapper;

    private RequirementService service;

    @BeforeEach
    void setup() {
        service = spy(new RequirementService(audit));
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

    private Requirement validDraft() {
        Requirement r = new Requirement();
        r.setProjectId(1L);
        r.setTitle("Test Requirement");
        r.setType("feature");
        r.setPriority("p1");
        r.setStatus("open");
        return r;
    }

    // ---- enum validation (throws before save, no DB mocking needed) ----

    @Test
    void create_throwsOnInvalidType() {
        Requirement r = validDraft();
        r.setType("not-a-valid-type");
        assertThrows(IllegalArgumentException.class, () -> service.create(r));
        verify(service, never()).save(any());
    }

    @Test
    void create_throwsOnInvalidPriority() {
        Requirement r = validDraft();
        r.setPriority("urgent");
        assertThrows(IllegalArgumentException.class, () -> service.create(r));
        verify(service, never()).save(any());
    }

    @Test
    void create_throwsOnInvalidStatus() {
        Requirement r = validDraft();
        r.setStatus("pending_review");
        assertThrows(IllegalArgumentException.class, () -> service.create(r));
        verify(service, never()).save(any());
    }

    // ---- default value injection ----

    @Test
    void create_setsDefaultPriorityP2AndStatusDraftWhenNull() {
        setPrincipal(1L, "dev");

        Requirement r = validDraft();
        r.setPriority(null);
        r.setStatus(null);

        doReturn(true).when(service).save(any(Requirement.class));

        Requirement result = service.create(r);
        assertEquals("p2", result.getPriority());
        assertEquals("draft", result.getStatus());
    }

    // ---- CurrentActor integration ----

    @Test
    void create_fillsCreatedByFromPrincipal() {
        setPrincipal(77L, "bob");

        Requirement r = validDraft();

        doReturn(true).when(service).save(any(Requirement.class));

        Requirement result = service.create(r);
        assertEquals(77L, result.getCreatedBy());
        verify(audit).report(eq("bob"), eq("PM_REQUIREMENT_CREATED"), anyString(), anyString(), any());
    }

    @Test
    void create_doesNotOverrideExplicitlySetCreatedBy() {
        setPrincipal(77L, "bob");

        Requirement r = validDraft();
        r.setCreatedBy(5L);

        doReturn(true).when(service).save(any(Requirement.class));

        Requirement result = service.create(r);
        assertEquals(5L, result.getCreatedBy());
    }

    @Test
    void create_usesSystemActorWhenNoPrincipal() {
        Requirement r = validDraft();

        doReturn(true).when(service).save(any(Requirement.class));

        service.create(r);
        verify(audit).report(eq("system"), eq("PM_REQUIREMENT_CREATED"), anyString(), anyString(), any());
    }

    // ---- update ----

    @Test
    void update_throwsNotFoundWhenRequirementMissing() {
        doReturn(null).when(service).getById(999L);
        assertThrows(NotFoundException.class, () -> service.update(999, new Requirement()));
    }
}
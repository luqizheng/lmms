package com.coder.pm.infra.service;

import com.coder.pm.domain.security.Principal;
import com.coder.pm.infra.persistence.entity.Task;
import com.coder.pm.infra.persistence.mapper.TaskMapper;
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
import static org.mockito.ArgumentMatchers.anyLong;
import static org.mockito.Mockito.*;

@ExtendWith(MockitoExtension.class)
class TaskServiceTest {

    @Mock private AuditEventReporter audit;
    @Mock private ProjectService projectService;
    @Mock private TaskMapper mapper;

    private TaskService service;

    @BeforeEach
    void setup() {
        service = spy(new TaskService(audit, projectService));
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

    private Task validDraft() {
        Task t = new Task();
        t.setProjectId(1L);
        t.setTitle("Implement login");
        return t;
    }

    @Test
    void create_throwsOnInvalidStatus() {
        Task t = validDraft();
        t.setStatus("not-a-status");
        assertThrows(IllegalArgumentException.class, () -> service.create(t));
    }

    @Test
    void create_throwsOnInvalidPriority() {
        Task t = validDraft();
        t.setPriority("urgent");
        assertThrows(IllegalArgumentException.class, () -> service.create(t));
    }

    @Test
    void create_setsDefaultsAndFillsCreatedByFromPrincipal() {
        setPrincipal(42L, "alice");
        Task t = validDraft();
        doAnswer(inv -> { ((Task) inv.getArgument(0)).setId(7L); return true; })
                .when(service).save(any(Task.class));
        Task result = service.create(t);
        assertEquals("todo", result.getStatus());
        assertEquals("medium", result.getPriority());
        assertEquals(0, result.getSortOrder());
        assertEquals(42L, result.getCreatedBy());
        verify(audit).report(eq("alice"), eq("PM_TASK_CREATED"), anyString(), anyString(), any());
    }

    @Test
    void update_throwsNotFoundWhenMissing() {
        doReturn(null).when(service).getById(99L);
        assertThrows(NotFoundException.class, () -> service.update(99L, new Task()));
    }

    @Test
    void update_throwsNotFoundWhenSoftDeleted() {
        Task deleted = validDraft();
        deleted.setId(5L);
        deleted.setDeletedAt(java.time.LocalDateTime.now());
        doReturn(deleted).when(service).getById(5L);
        assertThrows(NotFoundException.class, () -> service.update(5L, new Task()));
    }

    @Test
    void update_appliesPatchesAndFiresAudit() {
        setPrincipal(1L, "dev");
        Task existing = validDraft();
        existing.setId(10L);
        existing.setStatus("todo");
        existing.setPriority("medium");

        Task patch = new Task();
        patch.setStatus("in_progress");
        patch.setPriority("high");

        doReturn(existing).when(service).getById(10L);
        doReturn(true).when(service).updateById(any(Task.class));

        Task result = service.update(10L, patch);
        assertEquals("in_progress", result.getStatus());
        assertEquals("high", result.getPriority());
        verify(audit).report(eq("dev"), eq("PM_TASK_UPDATED"), anyString(), anyString(), any());
    }
}
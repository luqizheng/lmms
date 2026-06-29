package com.kudi.coder.demowebapp.service;

import com.kudi.coder.demowebapp.audit.AuditPublisher;
import com.kudi.coder.demowebapp.domain.Task;
import com.kudi.coder.demowebapp.domain.TaskStatus;
import com.kudi.coder.demowebapp.repository.TaskRepository;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;
import org.mockito.ArgumentCaptor;

import java.util.Map;
import java.util.Optional;

import static org.junit.jupiter.api.Assertions.*;
import static org.mockito.ArgumentMatchers.any;
import static org.mockito.ArgumentMatchers.eq;
import static org.mockito.Mockito.*;

class TaskServiceTest {

    private TaskRepository repo;
    private AuditPublisher publisher;
    private TaskService svc;

    @BeforeEach
    void setUp() {
        repo = mock(TaskRepository.class);
        publisher = mock(AuditPublisher.class);
        svc = new TaskService(repo, publisher);
    }

    @Test
    void create_persists_with_status_CREATED() {
        when(repo.save(any(Task.class))).thenAnswer(inv -> {
            Task t = inv.getArgument(0);
            t.setId(1L);
            return t;
        });

        Task t = svc.create("title", "desc", "alice");

        assertEquals("title", t.getTitle());
        assertEquals(TaskStatus.CREATED, t.getStatus());
        assertEquals("alice", t.getAssigneeId());  // creator is initial assignee
        verify(repo).save(any(Task.class));
    }

    @Test
    void get_throws_when_not_found() {
        when(repo.findById(99L)).thenReturn(Optional.empty());
        assertThrows(IllegalArgumentException.class, () -> svc.get(99L));
    }

    @Test
    void assign_sets_assignee_and_status_ASSIGNED() {
        Task t = new Task();
        t.setId(1L);
        t.setStatus(TaskStatus.CREATED);
        when(repo.findById(1L)).thenReturn(Optional.of(t));
        when(repo.save(any(Task.class))).thenAnswer(inv -> inv.getArgument(0));

        Task out = svc.assign(1L, "bob");

        assertEquals("bob", out.getAssigneeId());
        assertEquals(TaskStatus.ASSIGNED, out.getStatus());
    }

    @Test
    void changeStatus_to_DONE_from_CREATED_throws() {
        Task t = new Task();
        t.setId(1L);
        t.setStatus(TaskStatus.CREATED);
        when(repo.findById(1L)).thenReturn(Optional.of(t));

        assertThrows(IllegalStateException.class, () -> svc.changeStatus(1L, TaskStatus.DONE));
    }

    @Test
    void changeStatus_DONE_to_IN_PROGRESS_throws() {
        Task t = new Task();
        t.setId(1L);
        t.setStatus(TaskStatus.DONE);
        when(repo.findById(1L)).thenReturn(Optional.of(t));

        assertThrows(IllegalStateException.class, () -> svc.changeStatus(1L, TaskStatus.IN_PROGRESS));
    }

    @Test
    void delete_calls_repo() {
        when(repo.existsById(1L)).thenReturn(true);
        svc.delete(1L);
        verify(repo).deleteById(1L);
    }

    @Test
    void create_calls_auditPublisher() {
        when(repo.save(any(Task.class))).thenAnswer(inv -> {
            Task t = inv.getArgument(0);
            t.setId(7L);
            return t;
        });

        svc.create("title", "desc", "alice");

        ArgumentCaptor<Map<String, Object>> payloadCap = ArgumentCaptor.forClass(Map.class);
        verify(publisher).publish(eq("alice"), eq("TASK_CREATED"), eq("task"), eq("7"), payloadCap.capture());
        assertEquals("title", payloadCap.getValue().get("title"));
        assertEquals("CREATED", payloadCap.getValue().get("status"));
    }
}

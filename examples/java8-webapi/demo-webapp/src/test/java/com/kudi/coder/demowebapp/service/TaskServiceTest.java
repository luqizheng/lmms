package com.kudi.coder.demowebapp.service;

import com.kudi.coder.demowebapp.domain.Task;
import com.kudi.coder.demowebapp.domain.TaskStatus;
import com.kudi.coder.demowebapp.repository.TaskRepository;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;

import java.util.Optional;

import static org.junit.jupiter.api.Assertions.*;
import static org.mockito.ArgumentMatchers.any;
import static org.mockito.Mockito.*;

class TaskServiceTest {

    private TaskRepository repo;
    private TaskService svc;

    @BeforeEach
    void setUp() {
        repo = mock(TaskRepository.class);
        svc = new TaskService(repo);
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
}

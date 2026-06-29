package com.kudi.coder.demowebapp.service;

import com.kudi.coder.demowebapp.domain.Task;
import com.kudi.coder.demowebapp.domain.TaskStatus;
import com.kudi.coder.demowebapp.repository.TaskRepository;
import org.springframework.stereotype.Service;

import java.util.List;

@Service
public class TaskService {

    private final TaskRepository repo;

    public TaskService(TaskRepository repo) {
        this.repo = repo;
    }

    public Task create(String title, String description, String creatorId) {
        if (title == null || title.isEmpty()) {
            throw new IllegalArgumentException("title is required");
        }
        Task t = new Task();
        t.setTitle(title);
        t.setDescription(description);
        t.setStatus(TaskStatus.CREATED);
        t.setAssigneeId(creatorId);
        return repo.save(t);
    }

    public Task get(Long id) {
        return repo.findById(id)
                .orElseThrow(() -> new IllegalArgumentException("task " + id + " not found"));
    }

    public List<Task> list() {
        return repo.findAll();
    }

    public Task assign(Long id, String assigneeId) {
        if (assigneeId == null || assigneeId.isEmpty()) {
            throw new IllegalArgumentException("assigneeId is required");
        }
        Task t = get(id);
        if (t.getStatus() == TaskStatus.DONE) {
            throw new IllegalStateException("cannot assign a DONE task");
        }
        t.setAssigneeId(assigneeId);
        if (t.getStatus() == TaskStatus.CREATED) {
            t.setStatus(TaskStatus.ASSIGNED);
        }
        return repo.save(t);
    }

    public Task changeStatus(Long id, TaskStatus to) {
        Task t = get(id);
        validateTransition(t.getStatus(), to);
        t.setStatus(to);
        return repo.save(t);
    }

    public void delete(Long id) {
        if (!repo.existsById(id)) {
            throw new IllegalArgumentException("task " + id + " not found");
        }
        repo.deleteById(id);
    }

    private void validateTransition(TaskStatus from, TaskStatus to) {
        if (from == to) return;
        if (from == TaskStatus.DONE) {
            throw new IllegalStateException("DONE is terminal");
        }
        if (from == TaskStatus.CREATED && to == TaskStatus.DONE) {
            throw new IllegalStateException("cannot skip to DONE from CREATED");
        }
        if (from == TaskStatus.CREATED && to == TaskStatus.ASSIGNED) {
            throw new IllegalStateException("use assign() to set ASSIGNED");
        }
    }
}

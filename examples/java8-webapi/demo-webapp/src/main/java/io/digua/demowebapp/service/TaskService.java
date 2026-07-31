package io.digua.service;

import io.digua.AuditPublisher;
import io.digua.Task;
import io.digua.TaskStatus;
import io.digua.TaskRepository;
import org.springframework.stereotype.Service;

import java.util.Collections;
import java.util.HashMap;
import java.util.List;
import java.util.Map;

@Service
public class TaskService {

    private final TaskRepository repo;
    private final AuditPublisher publisher;

    public TaskService(TaskRepository repo, AuditPublisher publisher) {
        this.repo = repo;
        this.publisher = publisher;
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
        Task saved = repo.save(t);
        Map<String, Object> payload = new HashMap<>();
        payload.put("title", saved.getTitle());
        payload.put("status", saved.getStatus().name());
        publisher.publish(creatorId, "TASK_CREATED", "task", String.valueOf(saved.getId()), payload);
        return saved;
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
        TaskStatus from = t.getStatus();
        t.setAssigneeId(assigneeId);
        if (t.getStatus() == TaskStatus.CREATED) {
            t.setStatus(TaskStatus.ASSIGNED);
        }
        Task saved = repo.save(t);
        Map<String, Object> payload = new HashMap<>();
        payload.put("from", from.name());
        payload.put("to", saved.getStatus().name());
        payload.put("assigneeId", assigneeId);
        publisher.publish(assigneeId, "TASK_ASSIGNED", "task", String.valueOf(saved.getId()), payload);
        return saved;
    }

    public Task changeStatus(Long id, TaskStatus to) {
        Task t = get(id);
        TaskStatus from = t.getStatus();
        validateTransition(from, to);
        t.setStatus(to);
        Task saved = repo.save(t);
        // ponytail: actor=null until TaskController passes Principal into service
        Map<String, Object> payload = new HashMap<>();
        payload.put("from", from.name());
        payload.put("to", saved.getStatus().name());
        publisher.publish(null, "TASK_STATUS_CHANGED", "task", String.valueOf(saved.getId()), payload);
        return saved;
    }

    public void delete(Long id) {
        if (!repo.existsById(id)) {
            throw new IllegalArgumentException("task " + id + " not found");
        }
        repo.deleteById(id);
        // ponytail: actor=null until TaskController passes Principal into service
        publisher.publish(null, "TASK_DELETED", "task", String.valueOf(id), Collections.<String, Object>emptyMap());
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

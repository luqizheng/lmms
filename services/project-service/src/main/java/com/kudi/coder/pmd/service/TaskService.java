package com.kudi.coder.pmd.service;

import com.kudi.coder.pmd.config.TenantContext;
import com.kudi.coder.pmd.domain.Task;
import com.kudi.coder.pmd.domain.TaskStatus;
import com.kudi.coder.pmd.outbox.OutboxWriter;
import com.kudi.coder.pmd.repo.TaskRepository;
import com.kudi.coder.pmd.web.AppException;
import org.springframework.data.domain.Page;
import org.springframework.data.domain.Pageable;
import org.springframework.stereotype.Service;
import org.springframework.transaction.annotation.Transactional;

import java.math.BigDecimal;
import java.time.LocalDateTime;
import java.util.HashMap;
import java.util.Map;

@Service
public class TaskService {

    private final TaskRepository repository;
    private final OutboxWriter outbox;

    public TaskService(TaskRepository repository, OutboxWriter outbox) {
        this.repository = repository;
        this.outbox = outbox;
    }

    @Transactional
    public Task create(CreateRequest req) {
        Long tenantId = TenantContext.requireTenantId();
        Task t = new Task();
        t.setTenantId(tenantId);
        t.setProjectId(req.projectId);
        t.setMilestoneId(req.milestoneId);
        t.setSprintId(req.sprintId);
        t.setCode(req.code);
        t.setTitle(req.title);
        t.setDescription(req.description);
        t.setAssigneeId(req.assigneeId);
        t.setEstimateHours(req.estimateHours);
        t.setActorId(TenantContext.getUserId());
        t.setStatus(TaskStatus.Backlog);
        t = repository.save(t);
        outbox.enqueue("pmd.task.created", envelopeFor(t, "created", null));
        return t;
    }

    @Transactional(readOnly = true)
    public Task get(Long id) {
        Long tenantId = TenantContext.requireTenantId();
        return repository.findByIdAndTenantId(id, tenantId)
                .orElseThrow(() -> AppException.notFound("PMD_TASK_1003", "task", id));
    }

    @Transactional(readOnly = true)
    public Page<Task> list(Pageable pageable) {
        Long tenantId = TenantContext.requireTenantId();
        return repository.findAllByTenantId(tenantId, pageable);
    }

    @Transactional
    public Task updateStatus(Long id, String newStatusRaw) {
        Long tenantId = TenantContext.requireTenantId();
        Task t = get(id);
        TaskStatus newStatus;
        try {
            newStatus = TaskStatus.valueOf(newStatusRaw);
        } catch (IllegalArgumentException e) {
            throw AppException.badRequest("VALID_2001",
                    "unknown task status: " + newStatusRaw);
        }
        TaskStatus from = t.getStatus();
        TaskStatus.tryTransition(from, newStatus);
        // After tryTransition validates semantics, push to DB and emit event.
        repository.transitionStatus(id, tenantId, newStatus);
        // Re-fetch to capture updated_at
        Task updated = repository.findByIdAndTenantId(id, tenantId).orElseThrow();
        outbox.enqueue("pmd.task.status_changed",
                envelopeFor(updated, "status_changed", Pair.of(from, newStatus)));
        return updated;
    }

    private Map<String, Object> envelopeFor(Task t, String action, Pair<TaskStatus, TaskStatus> transition) {
        Map<String, Object> env = new HashMap<>();
        env.put("task_id", String.valueOf(t.getId()));
        env.put("project_id", String.valueOf(t.getProjectId()));
        env.put("code", t.getCode());
        env.put("status", t.getStatus().name());
        env.put("tenant_id", t.getTenantId());
        env.put("actor_id", TenantContext.getUserId());
        env.put("action", action);
        env.put("ts", LocalDateTime.now().toString());
        if (transition != null) {
            env.put("from_status", transition.first.name());
            env.put("to_status", transition.second.name());
        }
        return env;
    }

    public record CreateRequest(Long projectId,
                                Long milestoneId,
                                Long sprintId,
                                String code,
                                String title,
                                String description,
                                String assigneeId,
                                BigDecimal estimateHours) { }

    private record Pair<A, B>(A first, B second) { }
}

package com.coder.pm.infra.service;

import com.baomidou.mybatisplus.core.conditions.query.QueryWrapper;
import com.baomidou.mybatisplus.core.metadata.IPage;
import com.baomidou.mybatisplus.extension.plugins.pagination.Page;
import com.baomidou.mybatisplus.extension.service.impl.ServiceImpl;
import com.coder.pm.infra.persistence.entity.Task;
import com.coder.pm.infra.persistence.mapper.TaskMapper;
import com.coder.pm.infra.sdk.AuditEventReporter;
import com.coder.pm.infra.security.CurrentActor;
import com.coder.pm.web.exception.NotFoundException;
import org.springframework.stereotype.Service;
import org.springframework.transaction.annotation.Transactional;

import java.util.LinkedHashMap;
import java.util.Map;
import java.util.Set;

/**
 * Task domain service. Phase D4: simple standalone work items scoped to a
 * project (optionally linked to a requirement) with assignee, priority, status,
 * and due date. Status lifecycle is linear (todo -> in_progress -> done);
 * patches that move backward are allowed (e.g. un-do).
 */
@Service
public class TaskService extends ServiceImpl<TaskMapper, Task> {

    private static final Set<String> VALID_STATUSES =
            Set.of("todo", "in_progress", "done");
    private static final Set<String> VALID_PRIORITIES =
            Set.of("low", "medium", "high");

    private final AuditEventReporter audit;
    private final ProjectService projectService;

    public TaskService(AuditEventReporter audit, ProjectService projectService) {
        this.audit = audit;
        this.projectService = projectService;
    }

    @Transactional
    public Task create(Task draft) {
        // validate project exists
        projectService.findActiveOrThrow(draft.getProjectId());

        // default status
        if (draft.getStatus() == null) {
            draft.setStatus("todo");
        } else if (!VALID_STATUSES.contains(draft.getStatus())) {
            throw new IllegalArgumentException("invalid status: " + draft.getStatus()
                    + " (allowed: " + VALID_STATUSES + ")");
        }
        if (draft.getPriority() == null) {
            draft.setPriority("medium");
        } else if (!VALID_PRIORITIES.contains(draft.getPriority())) {
            throw new IllegalArgumentException("invalid priority: " + draft.getPriority()
                    + " (allowed: " + VALID_PRIORITIES + ")");
        }
        if (draft.getSortOrder() == null) {
            draft.setSortOrder(0);
        }
        if (draft.getCreatedBy() == null) {
            Long actorId = CurrentActor.actorIdOrNull();
            if (actorId != null) draft.setCreatedBy(actorId);
        }
        save(draft);
        audit.report(CurrentActor.displayName(), "PM_TASK_CREATED", "pm_task",
                String.valueOf(draft.getId()),
                Map.of("projectId", draft.getProjectId(),
                        "title", draft.getTitle(),
                        "status", draft.getStatus(),
                        "priority", draft.getPriority()));
        return draft;
    }

    @Transactional
    public Task update(long id, Task patch) {
        Task existing = getById(id);
        if (existing == null || existing.getDeletedAt() != null) {
            throw new NotFoundException("task not found: " + id);
        }
        if (patch.getTitle() != null) existing.setTitle(patch.getTitle());
        if (patch.getDescription() != null) existing.setDescription(patch.getDescription());
        if (patch.getStatus() != null) {
            if (!VALID_STATUSES.contains(patch.getStatus())) {
                throw new IllegalArgumentException("invalid status: " + patch.getStatus());
            }
            existing.setStatus(patch.getStatus());
        }
        if (patch.getPriority() != null) {
            if (!VALID_PRIORITIES.contains(patch.getPriority())) {
                throw new IllegalArgumentException("invalid priority: " + patch.getPriority());
            }
            existing.setPriority(patch.getPriority());
        }
        if (patch.getAssigneeId() != null) existing.setAssigneeId(patch.getAssigneeId());
        if (patch.getDueDate() != null) existing.setDueDate(patch.getDueDate());
        if (patch.getSortOrder() != null) existing.setSortOrder(patch.getSortOrder());
        updateById(existing);
        Map<String, Object> payload = new LinkedHashMap<>();
        payload.put("status", existing.getStatus());
        payload.put("priority", existing.getPriority());
        audit.report(CurrentActor.displayName(), "PM_TASK_UPDATED", "pm_task",
                String.valueOf(id), payload);
        return existing;
    }

    public Task findOrThrow(long id) {
        Task t = getById(id);
        if (t == null || t.getDeletedAt() != null) {
            throw new NotFoundException("task not found: " + id);
        }
        return t;
    }

    public IPage<Task> pageByProject(long projectId, String status, String priority,
                                     Long assigneeId, int page, int size) {
        QueryWrapper<Task> q = new QueryWrapper<>();
        q.eq("project_id", projectId).isNull("deleted_at");
        if (status != null && !status.isBlank()) q.eq("status", status);
        if (priority != null && !priority.isBlank()) q.eq("priority", priority);
        if (assigneeId != null) q.eq("assignee_id", assigneeId);
        q.orderByAsc("sort_order").orderByDesc("created_at");
        return page(new Page<>(page, size), q);
    }
}

package com.coder.pm.infra.service;

import com.baomidou.mybatisplus.core.conditions.query.QueryWrapper;
import com.baomidou.mybatisplus.core.metadata.IPage;
import com.baomidou.mybatisplus.extension.plugins.pagination.Page;
import com.baomidou.mybatisplus.extension.service.impl.ServiceImpl;
import com.coder.pm.infra.persistence.entity.Sprint;
import com.coder.pm.infra.persistence.mapper.SprintMapper;
import com.coder.pm.infra.sdk.AuditEventReporter;
import com.coder.pm.infra.security.CurrentActor;
import com.coder.pm.web.exception.NotFoundException;
import org.springframework.stereotype.Service;
import org.springframework.transaction.annotation.Transactional;

import java.util.LinkedHashMap;
import java.util.Map;
import java.util.Set;

/**
 * Sprint domain service. Phase D5: time-boxed iterations (planning -> active ->
 * completed) that group tasks. Tasks are linked to sprints via the nullable
 * {@code sprint_id} column on pm_tasks (V4 migration).
 */
@Service
public class SprintService extends ServiceImpl<SprintMapper, Sprint> {

    private static final Set<String> VALID_STATUSES =
            Set.of("planning", "active", "completed");

    private final AuditEventReporter audit;
    private final ProjectService projectService;

    public SprintService(AuditEventReporter audit, ProjectService projectService) {
        this.audit = audit;
        this.projectService = projectService;
    }

    @Transactional
    public Sprint create(Sprint draft) {
        projectService.findActiveOrThrow(draft.getProjectId());

        if (draft.getStatus() == null) {
            draft.setStatus("planning");
        } else if (!VALID_STATUSES.contains(draft.getStatus())) {
            throw new IllegalArgumentException("invalid status: " + draft.getStatus()
                    + " (allowed: " + VALID_STATUSES + ")");
        }
        if (draft.getCreatedBy() == null) {
            Long actorId = CurrentActor.actorIdOrNull();
            if (actorId != null) draft.setCreatedBy(actorId);
        }
        save(draft);
        audit.report(CurrentActor.displayName(), "PM_SPRINT_CREATED", "pm_sprint",
                String.valueOf(draft.getId()),
                Map.of("projectId", draft.getProjectId(),
                        "name", draft.getName(),
                        "status", draft.getStatus()));
        return draft;
    }

    @Transactional
    public Sprint update(long id, Sprint patch) {
        Sprint existing = getById(id);
        if (existing == null || existing.getDeletedAt() != null) {
            throw new NotFoundException("sprint not found: " + id);
        }
        if (patch.getName() != null) existing.setName(patch.getName());
        if (patch.getGoal() != null) existing.setGoal(patch.getGoal());
        if (patch.getStartDate() != null) existing.setStartDate(patch.getStartDate());
        if (patch.getEndDate() != null) existing.setEndDate(patch.getEndDate());
        if (patch.getStatus() != null) {
            if (!VALID_STATUSES.contains(patch.getStatus())) {
                throw new IllegalArgumentException("invalid status: " + patch.getStatus());
            }
            existing.setStatus(patch.getStatus());
        }
        updateById(existing);
        Map<String, Object> payload = new LinkedHashMap<>();
        payload.put("status", existing.getStatus());
        audit.report(CurrentActor.displayName(), "PM_SPRINT_UPDATED", "pm_sprint",
                String.valueOf(id), payload);
        return existing;
    }

    public Sprint findOrThrow(long id) {
        Sprint s = getById(id);
        if (s == null || s.getDeletedAt() != null) {
            throw new NotFoundException("sprint not found: " + id);
        }
        return s;
    }

    public IPage<Sprint> pageByProject(long projectId, String status,
                                       int page, int size) {
        QueryWrapper<Sprint> q = new QueryWrapper<>();
        q.eq("project_id", projectId).isNull("deleted_at");
        if (status != null && !status.isBlank()) q.eq("status", status);
        q.orderByDesc("created_at");
        return page(new Page<>(page, size), q);
    }
}

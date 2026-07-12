package com.coder.pm.infra.service;

import com.baomidou.mybatisplus.core.conditions.query.QueryWrapper;
import com.baomidou.mybatisplus.core.metadata.IPage;
import com.baomidou.mybatisplus.extension.plugins.pagination.Page;
import com.baomidou.mybatisplus.extension.service.impl.ServiceImpl;
import com.coder.pm.infra.persistence.entity.Requirement;
import com.coder.pm.infra.persistence.mapper.RequirementMapper;
import com.coder.pm.infra.sdk.AuditEventReporter;
import com.coder.pm.web.exception.NotFoundException;
import org.springframework.security.core.context.SecurityContextHolder;
import org.springframework.stereotype.Service;
import org.springframework.transaction.annotation.Transactional;

import java.util.LinkedHashMap;
import java.util.Map;
import java.util.Set;

/**
 * Requirement domain service. Lifecycle states are validated here; sibling/parent IDs are
 * sanity-checked but the hierarchical query is left to repositories (not part of MVP).
 *
 * <p>Audit events emitted on state transitions via {@link AuditEventReporter}
 * (failures are swallowed by the reporter).
 */
@Service
public class RequirementService extends ServiceImpl<RequirementMapper, Requirement> {

    private static final Set<String> VALID_TYPES = Set.of("feature", "improvement", "bug", "task");
    private static final Set<String> VALID_PRIORITIES = Set.of("p0", "p1", "p2", "p3");
    private static final Set<String> VALID_STATUSES = Set.of(
            "draft", "open", "in_progress", "resolved", "closed", "rejected");

    private final AuditEventReporter audit;

    public RequirementService(AuditEventReporter audit) {
        this.audit = audit;
    }

    @Transactional
    public Requirement create(Requirement draft) {
        validateEnum("type", draft.getType(), VALID_TYPES);
        if (draft.getPriority() == null) draft.setPriority("p2");
        if (draft.getStatus() == null) draft.setStatus("draft");
        validateEnum("priority", draft.getPriority(), VALID_PRIORITIES);
        validateEnum("status", draft.getStatus(), VALID_STATUSES);
        save(draft);
        audit.report(currentActor(), "PM_REQUIREMENT_CREATED", "pm_requirement",
                String.valueOf(draft.getId()),
                Map.of("projectId", draft.getProjectId(), "title", draft.getTitle(),
                        "type", draft.getType(), "priority", draft.getPriority()));
        return draft;
    }

    @Transactional
    public Requirement update(long id, Requirement patch) {
        Requirement existing = getById(id);
        if (existing == null) {
            throw new NotFoundException("requirement not found: " + id);
        }
        if (patch.getTitle() != null) existing.setTitle(patch.getTitle());
        if (patch.getDescription() != null) existing.setDescription(patch.getDescription());
        if (patch.getType() != null) {
            validateEnum("type", patch.getType(), VALID_TYPES);
            existing.setType(patch.getType());
        }
        if (patch.getPriority() != null) {
            validateEnum("priority", patch.getPriority(), VALID_PRIORITIES);
            existing.setPriority(patch.getPriority());
        }
        if (patch.getStatus() != null) {
            validateEnum("status", patch.getStatus(), VALID_STATUSES);
            existing.setStatus(patch.getStatus());
        }
        if (patch.getEstimateHours() != null) existing.setEstimateHours(patch.getEstimateHours());
        if (patch.getAssigneeId() != null) existing.setAssigneeId(patch.getAssigneeId());
        if (patch.getParentId() != null) existing.setParentId(patch.getParentId());
        updateById(existing);
        Map<String, Object> payload = new LinkedHashMap<>();
        payload.put("title", existing.getTitle());
        payload.put("status", existing.getStatus());
        payload.put("priority", existing.getPriority());
        audit.report(currentActor(), "PM_REQUIREMENT_UPDATED", "pm_requirement",
                String.valueOf(id), payload);
        return existing;
    }

    public Requirement findOrThrow(long id) {
        Requirement r = getById(id);
        if (r == null || r.getDeletedAt() != null) {
            throw new NotFoundException("requirement not found: " + id);
        }
        return r;
    }

    public IPage<Requirement> pageByProject(long projectId, String status, String type, int page, int size) {
        QueryWrapper<Requirement> q = new QueryWrapper<>();
        q.eq("project_id", projectId).isNull("deleted_at");
        if (status != null && !status.isBlank()) q.eq("status", status);
        if (type != null && !type.isBlank()) q.eq("type", type);
        q.orderByDesc("created_at");
        return page(new Page<>(page, size), q);
    }

    private void validateEnum(String field, String value, java.util.Collection<String> allowed) {
        if (value == null) return;
        if (!allowed.contains(value)) {
            throw new IllegalArgumentException("invalid " + field + ": " + value + " (allowed: " + allowed + ")");
        }
    }

    private static String currentActor() {
        var auth = SecurityContextHolder.getContext().getAuthentication();
        if (auth == null || auth.getPrincipal() == null || "anonymousUser".equals(auth.getPrincipal())) {
            return "system";
        }
        return String.valueOf(auth.getPrincipal());
    }
}

package com.coder.pm.infra.service;

import com.baomidou.mybatisplus.core.conditions.query.QueryWrapper;
import com.baomidou.mybatisplus.core.metadata.IPage;
import com.baomidou.mybatisplus.extension.plugins.pagination.Page;
import com.baomidou.mybatisplus.extension.service.impl.ServiceImpl;
import com.coder.pm.infra.persistence.entity.Project;
import com.coder.pm.infra.persistence.mapper.ProjectMapper;
import com.coder.pm.infra.sdk.AuditEventReporter;
import com.coder.pm.web.exception.ConflictException;
import com.coder.pm.web.exception.NotFoundException;
import org.springframework.security.core.context.SecurityContextHolder;
import org.springframework.stereotype.Service;
import org.springframework.transaction.annotation.Transactional;

import java.util.LinkedHashMap;
import java.util.Map;

/**
 * Project domain service. Wraps MyBatis-Plus {@link ServiceImpl} CRUD with two domain rules:
 * <ul>
 *   <li>{@code code} uniqueness (409 if duplicate)</li>
 *   <li>archival is a status transition to {@code archived}, not a hard delete</li>
 * </ul>
 *
 * <p>Audit events are emitted after successful state transitions via {@link AuditEventReporter}.
 * Failures there are swallowed by the reporter (MVP policy: audit must never block business).
 */
@Service
public class ProjectService extends ServiceImpl<ProjectMapper, Project> {

    private final AuditEventReporter audit;

    public ProjectService(AuditEventReporter audit) {
        this.audit = audit;
    }

    @Transactional
    public Project create(Project draft) {
        Long existing = lambdaQuery()
                .eq(Project::getCode, draft.getCode())
                .count();
        if (existing != null && existing > 0) {
            throw new ConflictException("project code already exists: " + draft.getCode());
        }
        if (draft.getStatus() == null) {
            draft.setStatus("active");
        }
        save(draft);
        audit.report(currentActor(), "PM_PROJECT_CREATED", "pm_project",
                String.valueOf(draft.getId()),
                Map.of("code", draft.getCode(), "name", draft.getName()));
        return draft;
    }

    @Transactional
    public Project update(long id, Project patch) {
        Project existing = getById(id);
        if (existing == null) {
            throw new NotFoundException("project not found: " + id);
        }
        if (patch.getName() != null) existing.setName(patch.getName());
        if (patch.getDescription() != null) existing.setDescription(patch.getDescription());
        if (patch.getOwnerId() != null) existing.setOwnerId(patch.getOwnerId());
        if (patch.getStatus() != null) {
            ensureValidStatusTransition(existing.getStatus(), patch.getStatus());
            existing.setStatus(patch.getStatus());
        }
        updateById(existing);
        Map<String, Object> payload = new LinkedHashMap<>();
        payload.put("name", existing.getName());
        payload.put("status", existing.getStatus());
        audit.report(currentActor(), "PM_PROJECT_UPDATED", "pm_project",
                String.valueOf(id), payload);
        return existing;
    }

    @Transactional
    public void archive(long id) {
        Project p = getById(id);
        if (p == null) {
            throw new NotFoundException("project not found: " + id);
        }
        p.setStatus("archived");
        updateById(p);
        audit.report(currentActor(), "PM_PROJECT_ARCHIVED", "pm_project",
                String.valueOf(id), Map.of("code", p.getCode()));
    }

    public Project findActiveOrThrow(long id) {
        Project p = getById(id);
        if (p == null || p.getDeletedAt() != null) {
            throw new NotFoundException("project not found: " + id);
        }
        return p;
    }

    public IPage<Project> page(int page, int size, String status) {
        QueryWrapper<Project> q = new QueryWrapper<>();
        q.isNull("deleted_at");
        if (status != null && !status.isBlank()) {
            q.eq("status", status);
        }
        q.orderByDesc("created_at");
        return page(new Page<>(page, size), q);
    }

    private void ensureValidStatusTransition(String from, String to) {
        if ("archived".equals(from) && !"active".equals(to)) return;
        if ("active".equals(from) && ("active".equals(to) || "archived".equals(to))) return;
        throw new ConflictException("invalid status transition: " + from + " -> " + to);
    }

    private static String currentActor() {
        var auth = SecurityContextHolder.getContext().getAuthentication();
        if (auth == null || auth.getPrincipal() == null || "anonymousUser".equals(auth.getPrincipal())) {
            return "system";
        }
        return String.valueOf(auth.getPrincipal());
    }
}

package com.coder.pm.infra.service;

import com.baomidou.mybatisplus.core.conditions.query.QueryWrapper;
import com.baomidou.mybatisplus.core.metadata.IPage;
import com.baomidou.mybatisplus.extension.plugins.pagination.Page;
import com.baomidou.mybatisplus.extension.service.impl.ServiceImpl;
import com.coder.pm.infra.persistence.entity.KbSpace;
import com.coder.pm.infra.persistence.mapper.KbSpaceMapper;
import com.coder.pm.infra.sdk.AuditEventReporter;
import com.coder.pm.web.exception.NotFoundException;
import org.springframework.security.core.context.SecurityContextHolder;
import org.springframework.stereotype.Service;
import org.springframework.transaction.annotation.Transactional;

import java.util.LinkedHashMap;
import java.util.Map;
import java.util.Set;

@Service
public class KbSpaceService extends ServiceImpl<KbSpaceMapper, KbSpace> {

    private static final Set<String> VALID_VISIBILITY = Set.of("private", "project", "open");

    private final AuditEventReporter audit;

    public KbSpaceService(AuditEventReporter audit) {
        this.audit = audit;
    }

    @Transactional
    public KbSpace create(KbSpace draft) {
        if (draft.getVisibility() == null) draft.setVisibility("project");
        else if (!VALID_VISIBILITY.contains(draft.getVisibility())) {
            throw new IllegalArgumentException(
                    "invalid visibility: " + draft.getVisibility() + " (allowed: " + VALID_VISIBILITY + ")");
        }
        save(draft);
        audit.report(currentActor(), "KB_SPACE_CREATED", "kb_space",
                String.valueOf(draft.getId()),
                Map.of("name", draft.getName(), "visibility", draft.getVisibility(),
                        "projectId", String.valueOf(draft.getProjectId())));
        return draft;
    }

    @Transactional
    public KbSpace update(long id, KbSpace patch) {
        KbSpace existing = getById(id);
        if (existing == null) throw new NotFoundException("kb space not found: " + id);
        if (patch.getName() != null) existing.setName(patch.getName());
        if (patch.getDescription() != null) existing.setDescription(patch.getDescription());
        if (patch.getVisibility() != null) {
            if (!VALID_VISIBILITY.contains(patch.getVisibility())) {
                throw new IllegalArgumentException(
                        "invalid visibility: " + patch.getVisibility());
            }
            existing.setVisibility(patch.getVisibility());
        }
        updateById(existing);
        Map<String, Object> payload = new LinkedHashMap<>();
        payload.put("name", existing.getName());
        payload.put("visibility", existing.getVisibility());
        audit.report(currentActor(), "KB_SPACE_UPDATED", "kb_space",
                String.valueOf(id), payload);
        return existing;
    }

    public KbSpace findActiveOrThrow(long id) {
        KbSpace s = getById(id);
        if (s == null || s.getDeletedAt() != null) {
            throw new NotFoundException("kb space not found: " + id);
        }
        return s;
    }

    public IPage<KbSpace> page(int page, int size, Long projectId, String visibility) {
        QueryWrapper<KbSpace> q = new QueryWrapper<>();
        q.isNull("deleted_at");
        if (projectId != null) q.eq("project_id", projectId);
        if (visibility != null && !visibility.isBlank()) q.eq("visibility", visibility);
        q.orderByDesc("created_at");
        return page(new Page<>(page, size), q);
    }

    private static String currentActor() {
        var auth = SecurityContextHolder.getContext().getAuthentication();
        if (auth == null || auth.getPrincipal() == null || "anonymousUser".equals(auth.getPrincipal())) {
            return "system";
        }
        return String.valueOf(auth.getPrincipal());
    }
}

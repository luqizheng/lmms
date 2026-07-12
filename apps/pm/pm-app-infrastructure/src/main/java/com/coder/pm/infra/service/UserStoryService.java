package com.coder.pm.infra.service;

import com.baomidou.mybatisplus.core.conditions.query.QueryWrapper;
import com.baomidou.mybatisplus.extension.service.impl.ServiceImpl;
import com.coder.pm.infra.persistence.entity.UserStory;
import com.coder.pm.infra.persistence.mapper.UserStoryMapper;
import com.coder.pm.infra.sdk.AuditEventReporter;
import com.coder.pm.web.exception.NotFoundException;
import org.springframework.security.core.context.SecurityContextHolder;
import org.springframework.stereotype.Service;
import org.springframework.transaction.annotation.Transactional;

import java.util.List;
import java.util.Map;
import java.util.Set;

@Service
public class UserStoryService extends ServiceImpl<UserStoryMapper, UserStory> {

    private static final Set<String> VALID_STATUSES = Set.of(
            "pending", "in_progress", "done", "rejected");

    private final AuditEventReporter audit;

    public UserStoryService(AuditEventReporter audit) {
        this.audit = audit;
    }

    @Transactional
    public UserStory create(UserStory draft) {
        if (draft.getStatus() == null) draft.setStatus("pending");
        else if (!VALID_STATUSES.contains(draft.getStatus())) {
            throw new IllegalArgumentException("invalid status: " + draft.getStatus());
        }
        if (draft.getSortOrder() == null) draft.setSortOrder(0);
        save(draft);
        audit.report(currentActor(), "PM_USER_STORY_CREATED", "pm_user_story",
                String.valueOf(draft.getId()),
                Map.of("requirementId", draft.getRequirementId(),
                        "iWant", String.valueOf(draft.getIWant()),
                        "status", draft.getStatus()));
        return draft;
    }

    @Transactional
    public UserStory update(long id, UserStory patch) {
        UserStory existing = getById(id);
        if (existing == null) throw new NotFoundException("user story not found: " + id);
        if (patch.getAsA() != null) existing.setAsA(patch.getAsA());
        if (patch.getIWant() != null) existing.setIWant(patch.getIWant());
        if (patch.getSoThat() != null) existing.setSoThat(patch.getSoThat());
        if (patch.getStoryPoints() != null) existing.setStoryPoints(patch.getStoryPoints());
        if (patch.getStatus() != null) {
            if (!VALID_STATUSES.contains(patch.getStatus())) {
                throw new IllegalArgumentException("invalid status: " + patch.getStatus());
            }
            existing.setStatus(patch.getStatus());
        }
        if (patch.getSortOrder() != null) existing.setSortOrder(patch.getSortOrder());
        updateById(existing);
        audit.report(currentActor(), "PM_USER_STORY_UPDATED", "pm_user_story",
                String.valueOf(id),
                Map.of("status", existing.getStatus(),
                        "storyPoints", String.valueOf(existing.getStoryPoints())));
        return existing;
    }

    public UserStory findOrThrow(long id) {
        UserStory s = getById(id);
        if (s == null) throw new NotFoundException("user story not found: " + id);
        return s;
    }

    public List<UserStory> listByRequirement(long requirementId) {
        QueryWrapper<UserStory> q = new QueryWrapper<>();
        q.eq("requirement_id", requirementId).orderByAsc("sort_order");
        return list(q);
    }

    private static String currentActor() {
        var auth = SecurityContextHolder.getContext().getAuthentication();
        if (auth == null || auth.getPrincipal() == null || "anonymousUser".equals(auth.getPrincipal())) {
            return "system";
        }
        return String.valueOf(auth.getPrincipal());
    }
}

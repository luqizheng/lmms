
package com.coder.pm.infra.service;

import com.baomidou.mybatisplus.core.conditions.query.QueryWrapper;
import com.baomidou.mybatisplus.extension.service.impl.ServiceImpl;
import com.coder.pm.infra.persistence.entity.AcceptanceCriteria;
import com.coder.pm.infra.persistence.mapper.AcceptanceCriteriaMapper;
import com.coder.pm.infra.sdk.AuditEventReporter;
import com.coder.pm.infra.security.CurrentActor;
import com.coder.pm.web.exception.NotFoundException;
import org.springframework.stereotype.Service;
import org.springframework.transaction.annotation.Transactional;

import java.util.List;
import java.util.Map;

@Service
public class AcceptanceCriteriaService extends ServiceImpl<AcceptanceCriteriaMapper, AcceptanceCriteria> {

    private final AuditEventReporter audit;

    public AcceptanceCriteriaService(AuditEventReporter audit) {
        this.audit = audit;
    }

    @Transactional
    public AcceptanceCriteria create(AcceptanceCriteria draft) {
        if (draft.getChecked() == null) draft.setChecked(false);
        if (draft.getSortOrder() == null) draft.setSortOrder(0);
        save(draft);
        audit.report(currentActor(), "PM_AC_CREATED", "pm_acceptance_criterion",
                String.valueOf(draft.getId()),
                Map.of("userStoryId", draft.getUserStoryId(),
                        "given", String.valueOf(draft.getGivenText()),
                        "then", String.valueOf(draft.getThenText())));
        return draft;
    }

    @Transactional
    public AcceptanceCriteria update(long id, AcceptanceCriteria patch) {
        AcceptanceCriteria existing = getById(id);
        if (existing == null) throw new NotFoundException("acceptance criterion not found: " + id);
        if (patch.getGivenText() != null) existing.setGivenText(patch.getGivenText());
        if (patch.getWhenText() != null) existing.setWhenText(patch.getWhenText());
        if (patch.getThenText() != null) existing.setThenText(patch.getThenText());
        if (patch.getChecked() != null) existing.setChecked(patch.getChecked());
        if (patch.getSortOrder() != null) existing.setSortOrder(patch.getSortOrder());
        updateById(existing);
        audit.report(currentActor(), "PM_AC_UPDATED", "pm_acceptance_criterion",
                String.valueOf(id),
                Map.of("checked", existing.getChecked()));
        return existing;
    }

    @Transactional
    public AcceptanceCriteria check(long id, boolean checked) {
        AcceptanceCriteria existing = getById(id);
        if (existing == null) throw new NotFoundException("acceptance criterion not found: " + id);
        existing.setChecked(checked);
        updateById(existing);
        audit.report(currentActor(),
                checked ? "PM_AC_CHECKED" : "PM_AC_UNCHECKED",
                "pm_acceptance_criterion",
                String.valueOf(id), Map.of());
        return existing;
    }

    public AcceptanceCriteria findOrThrow(long id) {
        AcceptanceCriteria a = getById(id);
        if (a == null) throw new NotFoundException("acceptance criterion not found: " + id);
        return a;
    }

    public List<AcceptanceCriteria> listByStory(long storyId) {
        QueryWrapper<AcceptanceCriteria> q = new QueryWrapper<>();
        q.eq("user_story_id", storyId).orderByAsc("sort_order");
        return list(q);
    }

    private static String currentActor() {
        return CurrentActor.displayName();
    }
}

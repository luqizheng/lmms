package com.kudi.coder.pmd.service;

import com.kudi.coder.pmd.config.TenantContext;
import com.kudi.coder.pmd.domain.SprintMapping;
import com.kudi.coder.pmd.repo.SprintRepository;
import com.kudi.coder.pmd.web.AppException;
import org.springframework.data.domain.Page;
import org.springframework.data.domain.Pageable;
import org.springframework.stereotype.Service;
import org.springframework.transaction.annotation.Transactional;

@Service
public class SprintService {

    private final SprintRepository repository;

    public SprintService(SprintRepository repository) {
        this.repository = repository;
    }

    @Transactional
    public SprintMapping create(CreateRequest req) {
        Long tenantId = TenantContext.requireTenantId();
        SprintMapping s = new SprintMapping();
        s.setTenantId(tenantId);
        s.setProjectId(req.projectId);
        s.setSprintName(req.sprintName);
        s.setStartsOn(req.startsOn);
        s.setEndsOn(req.endsOn);
        s.setActorId(TenantContext.getUserId());
        return repository.save(s);
    }

    @Transactional(readOnly = true)
    public Page<SprintMapping> list(Long projectId, Pageable pageable) {
        Long tenantId = TenantContext.requireTenantId();
        return projectId == null
                ? repository.findAllByTenantId(tenantId, pageable)
                : repository.findAllByTenantIdAndProjectId(tenantId, projectId, pageable);
    }

    @Transactional(readOnly = true)
    public SprintMapping get(Long id) {
        Long tenantId = TenantContext.requireTenantId();
        return repository.findByIdAndTenantId(id, tenantId)
                .orElseThrow(() -> AppException.notFound("PMD_SPRINT_1004", "sprint", id));
    }

    public record CreateRequest(Long projectId,
                                String sprintName,
                                java.time.LocalDate startsOn,
                                java.time.LocalDate endsOn) { }
}

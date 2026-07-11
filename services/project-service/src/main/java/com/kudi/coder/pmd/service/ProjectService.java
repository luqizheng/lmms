package com.kudi.coder.pmd.service;

import com.kudi.coder.pmd.config.TenantContext;
import com.kudi.coder.pmd.domain.Project;
import com.kudi.coder.pmd.outbox.OutboxWriter;
import com.kudi.coder.pmd.repo.ProjectRepository;
import com.kudi.coder.pmd.web.AppException;
import org.springframework.data.domain.Page;
import org.springframework.data.domain.Pageable;
import org.springframework.stereotype.Service;
import org.springframework.transaction.annotation.Transactional;

import java.time.LocalDateTime;
import java.util.HashMap;
import java.util.Map;

/**
 * Project CRUD. Every mutating method emits a domain event via {@link OutboxWriter}
 * in the same @Transactional boundary; reads are tenant-scoped via
 * repository finder methods that already encode `tenant_id` in their where clause.
 */
@Service
public class ProjectService {

    private final ProjectRepository repository;
    private final OutboxWriter outbox;

    public ProjectService(ProjectRepository repository, OutboxWriter outbox) {
        this.repository = repository;
        this.outbox = outbox;
    }

    @Transactional
    public Project create(CreateRequest req) {
        Long tenantId = TenantContext.requireTenantId();
        if (repository.existsByCodeAndTenantId(req.code, tenantId)) {
            throw AppException.conflict("PMD_PROJECT_3001",
                    "project code already exists: " + req.code);
        }
        Project p = new Project();
        p.setTenantId(tenantId);
        p.setCode(req.code);
        p.setName(req.name);
        p.setDescription(req.description);
        p.setOwnerId(req.ownerId);
        p.setStartDate(req.startDate);
        p.setEndDate(req.endDate);
        p.setActorId(TenantContext.getUserId());
        p = repository.save(p);

        outbox.enqueue("pmd.project.created", envelopeFor(p, "created"));
        return p;
    }

    @Transactional(readOnly = true)
    public Project get(Long id) {
        Long tenantId = TenantContext.requireTenantId();
        return repository.findByIdAndTenantId(id, tenantId)
                .orElseThrow(() -> AppException.notFound("PMD_PROJECT_1001", "project", id));
    }

    @Transactional(readOnly = true)
    public Page<Project> list(Pageable pageable) {
        Long tenantId = TenantContext.requireTenantId();
        return repository.findAllByTenantId(tenantId, pageable);
    }

    @Transactional
    public Project update(Long id, UpdateRequest req) {
        Project p = get(id);
        if (req.name != null) p.setName(req.name);
        if (req.description != null) p.setDescription(req.description);
        if (req.startDate != null) p.setStartDate(req.startDate);
        if (req.endDate != null) p.setEndDate(req.endDate);
        p.setActorId(TenantContext.getUserId());
        return p;
    }

    @Transactional
    public void archive(Long id) {
        Project p = get(id);
        p.setStatus(Project.ProjectStatus.archived);
        p.setActorId(TenantContext.getUserId());
        // outbox emit AFTER status change has been persisted within the same tx
        outbox.enqueue("pmd.project.archived", envelopeFor(p, "archived"));
    }

    private Map<String, Object> envelopeFor(Project p, String action) {
        Map<String, Object> m = new HashMap<>();
        m.put("project_id", String.valueOf(p.getId()));
        m.put("code", p.getCode());
        m.put("name", p.getName());
        m.put("owner_id", p.getOwnerId());
        m.put("tenant_id", p.getTenantId());
        m.put("actor_id", TenantContext.getUserId());
        m.put("action", action);
        m.put("ts", LocalDateTime.now().toString());
        return m;
    }

    public record CreateRequest(String code,
                                String name,
                                String description,
                                String ownerId,
                                java.time.LocalDate startDate,
                                java.time.LocalDate endDate) { }

    public record UpdateRequest(String name,
                                String description,
                                java.time.LocalDate startDate,
                                java.time.LocalDate endDate) { }
}

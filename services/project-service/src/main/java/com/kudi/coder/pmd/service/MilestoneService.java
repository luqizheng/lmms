package com.kudi.coder.pmd.service;

import com.kudi.coder.pmd.config.TenantContext;
import com.kudi.coder.pmd.domain.Milestone;
import com.kudi.coder.pmd.domain.Project;
import com.kudi.coder.pmd.outbox.OutboxWriter;
import com.kudi.coder.pmd.repo.MilestoneRepository;
import com.kudi.coder.pmd.repo.ProjectRepository;
import com.kudi.coder.pmd.web.AppException;
import org.springframework.data.domain.Page;
import org.springframework.data.domain.Pageable;
import org.springframework.stereotype.Service;
import org.springframework.transaction.annotation.Transactional;

import java.time.LocalDate;
import java.time.LocalDateTime;
import java.util.HashMap;
import java.util.Map;

@Service
public class MilestoneService {

    private final MilestoneRepository repository;
    private final ProjectRepository projectRepository;
    private final OutboxWriter outbox;

    public MilestoneService(MilestoneRepository repository,
                            ProjectRepository projectRepository,
                            OutboxWriter outbox) {
        this.repository = repository;
        this.projectRepository = projectRepository;
        this.outbox = outbox;
    }

    @Transactional
    public Milestone create(CreateRequest req) {
        Long tenantId = TenantContext.requireTenantId();
        // Tenant + project must align — fetch project under same tenant.
        Project project = projectRepository.findByIdAndTenantId(req.projectId, tenantId)
                .orElseThrow(() -> AppException.notFound(
                        "PMD_PROJECT_1001", "project", req.projectId));
        Milestone m = new Milestone();
        m.setTenantId(tenantId);
        m.setProjectId(project.getId());
        m.setCode(req.code);
        m.setName(req.name);
        m.setDueDate(req.dueDate);
        m.setActorId(TenantContext.getUserId());
        m = repository.save(m);
        outbox.enqueue("pmd.milestone.created", envelopeFor(m, "created"));
        return m;
    }

    @Transactional(readOnly = true)
    public Milestone get(Long id) {
        Long tenantId = TenantContext.requireTenantId();
        return repository.findByIdAndTenantId(id, tenantId)
                .orElseThrow(() -> AppException.notFound("PMD_MILESTONE_1002", "milestone", id));
    }

    @Transactional(readOnly = true)
    public Page<Milestone> list(Long projectId, Pageable pageable) {
        Long tenantId = TenantContext.requireTenantId();
        return repository.findAllByTenantIdAndProjectId(tenantId, projectId, pageable);
    }

    @Transactional
    public Milestone update(Long id, UpdateRequest req) {
        Milestone m = get(id);
        if (req.name != null) m.setName(req.name);
        if (req.dueDate != null) m.setDueDate(req.dueDate);
        if (req.status != null) m.setStatus(Milestone.MilestoneStatus.valueOf(req.status));
        m.setActorId(TenantContext.getUserId());
        return m;
    }

    private Map<String, Object> envelopeFor(Milestone m, String action) {
        Map<String, Object> env = new HashMap<>();
        env.put("milestone_id", String.valueOf(m.getId()));
        env.put("project_id", String.valueOf(m.getProjectId()));
        env.put("code", m.getCode());
        env.put("status", m.getStatus().name());
        env.put("tenant_id", m.getTenantId());
        env.put("actor_id", TenantContext.getUserId());
        env.put("action", action);
        env.put("ts", LocalDateTime.now().toString());
        return env;
    }

    public record CreateRequest(Long projectId,
                                String code,
                                String name,
                                LocalDate dueDate) { }

    public record UpdateRequest(String name,
                                LocalDate dueDate,
                                String status) { }
}

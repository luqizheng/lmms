package com.coder.pm.api.requirement;

import com.baomidou.mybatisplus.core.metadata.IPage;
import com.coder.pm.infra.persistence.entity.Requirement;
import com.coder.pm.infra.service.ProjectService;
import com.coder.pm.infra.service.RequirementService;
import io.swagger.v3.oas.annotations.Operation;
import io.swagger.v3.oas.annotations.tags.Tag;
import jakarta.validation.Valid;
import jakarta.validation.constraints.NotBlank;
import org.springframework.http.ResponseEntity;
import org.springframework.web.bind.annotation.DeleteMapping;
import org.springframework.web.bind.annotation.GetMapping;
import org.springframework.web.bind.annotation.PatchMapping;
import org.springframework.web.bind.annotation.PathVariable;
import org.springframework.web.bind.annotation.PostMapping;
import org.springframework.web.bind.annotation.RequestBody;
import org.springframework.web.bind.annotation.RequestMapping;
import org.springframework.web.bind.annotation.RequestParam;
import org.springframework.web.bind.annotation.RestController;

import java.util.Map;

/**
 * Requirement lifecycle. Nested under project for create + list, flat paths for detail / patch /
 * delete.
 */
@RestController
@Tag(name = "requirements", description = "PM requirement lifecycle")
public class RequirementController {

    private final RequirementService service;
    private final ProjectService projectService;

    public RequirementController(RequirementService service, ProjectService projectService) {
        this.service = service;
        this.projectService = projectService;
    }

    @PostMapping("/v1/projects/{projectId}/requirements")
    @Operation(summary = "Create requirement in project")
    public ResponseEntity<Requirement> create(@PathVariable long projectId,
                                              @RequestBody @Valid CreateRequirementRequest req) {
        projectService.findActiveOrThrow(projectId); // validate parent exists
        Requirement draft = new Requirement();
        draft.setProjectId(projectId);
        draft.setParentId(req.parentId());
        draft.setTitle(req.title());
        draft.setDescription(req.description());
        draft.setType(req.type());
        draft.setPriority(req.priority());
        draft.setStatus(req.status());
        draft.setEstimateHours(req.estimateHours());
        draft.setAssigneeId(req.assigneeId());
        Requirement saved = service.create(draft);
        return ResponseEntity.status(201).body(saved);
    }

    @GetMapping("/v1/projects/{projectId}/requirements")
    @Operation(summary = "Paged requirements for project")
    public Map<String, Object> list(@PathVariable long projectId,
                                    @RequestParam(defaultValue = "1") int page,
                                    @RequestParam(defaultValue = "20") int size,
                                    @RequestParam(required = false) String status,
                                    @RequestParam(required = false) String type) {
        IPage<Requirement> p = service.pageByProject(projectId, status, type, page, size);
        return Map.of(
                "items", p.getRecords(),
                "total", p.getTotal(),
                "page", p.getCurrent(),
                "size", p.getSize()
        );
    }

    @GetMapping("/v1/requirements/{id}")
    @Operation(summary = "Requirement detail")
    public Requirement detail(@PathVariable long id) {
        return service.findOrThrow(id);
    }

    @PatchMapping("/v1/requirements/{id}")
    @Operation(summary = "Patch requirement fields")
    public Requirement patch(@PathVariable long id, @RequestBody UpdateRequirementRequest req) {
        Requirement patch = new Requirement();
        patch.setTitle(req.title());
        patch.setDescription(req.description());
        patch.setType(req.type());
        patch.setPriority(req.priority());
        patch.setStatus(req.status());
        patch.setEstimateHours(req.estimateHours());
        patch.setAssigneeId(req.assigneeId());
        patch.setParentId(req.parentId());
        return service.update(id, patch);
    }

    @DeleteMapping("/v1/requirements/{id}")
    @Operation(summary = "Soft-delete requirement")
    public ResponseEntity<Void> delete(@PathVariable long id) {
        Requirement existing = service.findOrThrow(id);
        existing.setDeletedAt(java.time.LocalDateTime.now());
        service.updateById(existing);
        return ResponseEntity.noContent().build();
    }

    public record CreateRequirementRequest(
            @NotBlank String title,
            String description,
            @NotBlank String type,
            String priority,
            String status,
            Integer estimateHours,
            Long parentId,
            Long assigneeId
    ) {}

    public record UpdateRequirementRequest(
            String title,
            String description,
            String type,
            String priority,
            String status,
            Integer estimateHours,
            Long parentId,
            Long assigneeId
    ) {}
}

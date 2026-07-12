package com.coder.pm.api.project;

import com.baomidou.mybatisplus.core.metadata.IPage;
import com.coder.pm.infra.persistence.entity.Project;
import com.coder.pm.infra.service.ProjectService;
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
 * REST endpoints for {@link Project} lifecycle.
 *
 * <pre>
 * POST   /pm-api/v1/projects              create
 * GET    /pm-api/v1/projects              paged list (?page&size&status)
 * GET    /pm-api/v1/projects/{id}         detail
 * PATCH  /pm-api/v1/projects/{id}         patch (name/description/owner/status)
 * DELETE /pm-api/v1/projects/{id}         archive (soft-delete by status transition)
 * </pre>
 */
@RestController
@RequestMapping("/v1/projects")
@Tag(name = "projects", description = "PM project lifecycle")
public class ProjectController {

    private final ProjectService service;

    public ProjectController(ProjectService service) {
        this.service = service;
    }

    @PostMapping
    @Operation(summary = "Create a project")
    public ResponseEntity<Project> create(@RequestBody @Valid CreateProjectRequest req) {
        Project draft = new Project();
        draft.setCode(req.code());
        draft.setName(req.name());
        draft.setDescription(req.description());
        Long actorId = com.coder.pm.infra.security.CurrentActor.actorIdOrZero();
        draft.setOwnerId(req.ownerId() == null ? actorId : req.ownerId());
        draft.setCreatedBy(actorId);
        Project saved = service.create(draft);
        return ResponseEntity.status(201).body(saved);
    }

    @GetMapping
    @Operation(summary = "Paged project list")
    public Map<String, Object> list(@RequestParam(defaultValue = "1") int page,
                                    @RequestParam(defaultValue = "20") int size,
                                    @RequestParam(required = false) String status) {
        IPage<Project> p = service.page(page, size, status);
        return Map.of(
                "items", p.getRecords(),
                "total", p.getTotal(),
                "page", p.getCurrent(),
                "size", p.getSize()
        );
    }

    @GetMapping("/{id}")
    @Operation(summary = "Project detail")
    public Project detail(@PathVariable long id) {
        return service.findActiveOrThrow(id);
    }

    @PatchMapping("/{id}")
    @Operation(summary = "Patch project fields")
    public Project patch(@PathVariable long id, @RequestBody UpdateProjectRequest req) {
        Project patch = new Project();
        patch.setName(req.name());
        patch.setDescription(req.description());
        patch.setOwnerId(req.ownerId());
        patch.setStatus(req.status());
        return service.update(id, patch);
    }

    @DeleteMapping("/{id}")
    @Operation(summary = "Archive (soft-delete) project")
    public ResponseEntity<Void> archive(@PathVariable long id) {
        service.archive(id);
        return ResponseEntity.noContent().build();
    }

    public record CreateProjectRequest(
            @NotBlank String code,
            @NotBlank String name,
            String description,
            Long ownerId
    ) {}

    public record UpdateProjectRequest(
            String name,
            String description,
            Long ownerId,
            String status
    ) {}
}

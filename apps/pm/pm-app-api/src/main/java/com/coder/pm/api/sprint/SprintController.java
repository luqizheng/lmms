package com.coder.pm.api.sprint;

import com.baomidou.mybatisplus.core.metadata.IPage;
import com.coder.pm.infra.persistence.entity.Sprint;
import com.coder.pm.infra.service.ProjectService;
import com.coder.pm.infra.service.SprintService;
import io.swagger.v3.oas.annotations.Operation;
import io.swagger.v3.oas.annotations.tags.Tag;
import jakarta.validation.Valid;
import jakarta.validation.constraints.NotBlank;
import org.springframework.http.ResponseEntity;
import org.springframework.web.bind.annotation.*;

import java.time.LocalDate;
import java.util.Map;

/**
 * Sprint lifecycle. Phase D5: time-boxed iterations (planning -> active ->
 * completed) under a project. Tasks are linked via the nullable
 * {@code sprint_id} column on pm_tasks; this controller does not directly
 * manage task-sprint assignment (PATCH /v1/tasks/{id} with sprintId covers
 * that).
 *
 * <pre>
 * POST   /v1/projects/{projectId}/sprints    create
 * GET    /v1/projects/{projectId}/sprints    paged list (?status)
 * GET    /v1/sprints/{id}                     detail
 * PATCH  /v1/sprints/{id}                     patch (name/goal/startDate/endDate/status)
 * DELETE /v1/sprints/{id}                     soft-delete
 * </pre>
 */
@RestController
@Tag(name = "sprints", description = "PM sprint management (Phase D5)")
public class SprintController {

    private final SprintService service;
    private final ProjectService projectService;

    public SprintController(SprintService service, ProjectService projectService) {
        this.service = service;
        this.projectService = projectService;
    }

    @PostMapping("/v1/projects/{projectId}/sprints")
    @Operation(summary = "Create sprint in project")
    public ResponseEntity<Sprint> create(@PathVariable long projectId,
                                        @RequestBody @Valid CreateSprintRequest req) {
        projectService.findActiveOrThrow(projectId);
        Sprint s = new Sprint();
        s.setProjectId(projectId);
        s.setName(req.name());
        s.setGoal(req.goal());
        s.setStartDate(req.startDate());
        s.setEndDate(req.endDate());
        s.setStatus(req.status());
        Sprint saved = service.create(s);
        return ResponseEntity.status(201).body(saved);
    }

    @GetMapping("/v1/projects/{projectId}/sprints")
    @Operation(summary = "Paged sprints for project")
    public Map<String, Object> list(@PathVariable long projectId,
                                    @RequestParam(defaultValue = "1") int page,
                                    @RequestParam(defaultValue = "20") int size,
                                    @RequestParam(required = false) String status) {
        IPage<Sprint> p = service.pageByProject(projectId, status, page, size);
        return Map.of(
                "items", p.getRecords(),
                "total", p.getTotal(),
                "page", p.getCurrent(),
                "size", p.getSize()
        );
    }

    @GetMapping("/v1/sprints/{id}")
    @Operation(summary = "Sprint detail")
    public Sprint detail(@PathVariable long id) {
        return service.findOrThrow(id);
    }

    @PatchMapping("/v1/sprints/{id}")
    @Operation(summary = "Patch sprint fields")
    public Sprint patch(@PathVariable long id, @RequestBody UpdateSprintRequest req) {
        Sprint patch = new Sprint();
        patch.setName(req.name());
        patch.setGoal(req.goal());
        patch.setStartDate(req.startDate());
        patch.setEndDate(req.endDate());
        patch.setStatus(req.status());
        return service.update(id, patch);
    }

    @DeleteMapping("/v1/sprints/{id}")
    @Operation(summary = "Soft-delete sprint")
    public ResponseEntity<Void> delete(@PathVariable long id) {
        Sprint existing = service.findOrThrow(id);
        existing.setDeletedAt(java.time.LocalDateTime.now());
        service.updateById(existing);
        return ResponseEntity.noContent().build();
    }

    public record CreateSprintRequest(
            @NotBlank String name,
            String goal,
            String status,
            LocalDate startDate,
            LocalDate endDate
    ) {}

    public record UpdateSprintRequest(
            String name,
            String goal,
            String status,
            LocalDate startDate,
            LocalDate endDate
    ) {}
}

package com.kudi.coder.pmd.web;

import com.kudi.coder.pmd.service.ProjectService;
import com.kudi.coder.pmd.domain.Project;
import com.kudi.coder.pmd.config.TenantContext;
import io.swagger.v3.oas.annotations.Operation;
import io.swagger.v3.oas.annotations.responses.ApiResponse;
import io.swagger.v3.oas.annotations.tags.Tag;
import jakarta.validation.Valid;
import jakarta.validation.constraints.NotBlank;
import jakarta.validation.constraints.NotNull;
import org.springframework.data.domain.Page;
import org.springframework.data.domain.Pageable;
import org.springframework.http.ResponseEntity;
import org.springframework.web.bind.annotation.DeleteMapping;
import org.springframework.web.bind.annotation.GetMapping;
import org.springframework.web.bind.annotation.PatchMapping;
import org.springframework.web.bind.annotation.PathVariable;
import org.springframework.web.bind.annotation.PostMapping;
import org.springframework.web.bind.annotation.RequestBody;
import org.springframework.web.bind.annotation.RequestMapping;
import org.springframework.web.bind.annotation.RestController;

import java.net.URI;
import java.time.LocalDate;

@RestController
@RequestMapping("/projects")
@Tag(name = "projects", description = "PMD project CRUD")
public class ProjectController {

    private final ProjectService service;

    public ProjectController(ProjectService service) {
        this.service = service;
    }

    @PostMapping
    @Operation(summary = "Create project")
    @ApiResponse(responseCode = "201", description = "created")
    public ResponseEntity<Project> create(@Valid @RequestBody CreateBody body) {
        ProjectService.CreateRequest req = new ProjectService.CreateRequest(
                body.code(), body.name(), body.description(),
                body.ownerId(), body.startDate(), body.endDate());
        Project p = service.create(req);
        return ResponseEntity.created(URI.create("/projects/" + p.getId())).body(p);
    }

    @GetMapping
    @Operation(summary = "List projects (tenant scoped)")
    public Page<Project> list(Pageable pageable) {
        return service.list(pageable);
    }

    @GetMapping("/{id}")
    @Operation(summary = "Get project by id (tenant scoped)")
    public Project get(@PathVariable Long id) {
        return service.get(id);
    }

    @PatchMapping("/{id}")
    @Operation(summary = "Update project metadata")
    public Project update(@PathVariable Long id, @RequestBody UpdateBody body) {
        return service.update(id, new ProjectService.UpdateRequest(
                body.name(), body.description(), body.startDate(), body.endDate()));
    }

    @DeleteMapping("/{id}")
    @Operation(summary = "Archive project (soft delete)")
    public ResponseEntity<Void> archive(@PathVariable Long id) {
        service.archive(id);
        return ResponseEntity.noContent().build();
    }

    public record CreateBody(
            @NotBlank String code,
            @NotBlank String name,
            String description,
            @NotBlank String ownerId,
            LocalDate startDate,
            LocalDate endDate) { }

    public record UpdateBody(
            String name,
            String description,
            LocalDate startDate,
            LocalDate endDate) { }
}

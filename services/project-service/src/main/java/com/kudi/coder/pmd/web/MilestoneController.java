package com.kudi.coder.pmd.web;

import com.kudi.coder.pmd.service.MilestoneService;
import com.kudi.coder.pmd.domain.Milestone;
import io.swagger.v3.oas.annotations.Operation;
import io.swagger.v3.oas.annotations.tags.Tag;
import jakarta.validation.Valid;
import jakarta.validation.constraints.NotBlank;
import jakarta.validation.constraints.NotNull;
import org.springframework.data.domain.Page;
import org.springframework.data.domain.Pageable;
import org.springframework.http.ResponseEntity;
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
@RequestMapping("/projects/{projectId}/milestones")
@Tag(name = "milestones", description = "PMD milestone CRUD")
public class MilestoneController {

    private final MilestoneService service;

    public MilestoneController(MilestoneService service) {
        this.service = service;
    }

    @PostMapping
    @Operation(summary = "Create milestone under project")
    public ResponseEntity<Milestone> create(@PathVariable Long projectId,
                                            @Valid @RequestBody CreateBody body) {
        MilestoneService.CreateRequest req = new MilestoneService.CreateRequest(
                projectId, body.code(), body.name(), body.dueDate());
        Milestone m = service.create(req);
        return ResponseEntity.created(URI.create("/milestones/" + m.getId())).body(m);
    }

    @GetMapping
    @Operation(summary = "List milestones under project")
    public Page<Milestone> list(@PathVariable Long projectId, Pageable pageable) {
        return service.list(projectId, pageable);
    }

    @GetMapping("/{id}")
    @Operation(summary = "Get milestone by id")
    public Milestone get(@PathVariable Long id) {
        return service.get(id);
    }

    @PatchMapping("/{id}")
    @Operation(summary = "Update milestone (name / due_date / status)")
    public Milestone update(@PathVariable Long id, @RequestBody UpdateBody body) {
        return service.update(id, new MilestoneService.UpdateRequest(
                body.name(), body.dueDate(), body.status()));
    }

    public record CreateBody(
            @NotBlank String code,
            @NotBlank String name,
            LocalDate dueDate) { }

    public record UpdateBody(
            String name,
            LocalDate dueDate,
            String status) { }
}

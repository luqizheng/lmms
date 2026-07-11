package com.kudi.coder.pmd.web;

import com.kudi.coder.pmd.service.SprintService;
import com.kudi.coder.pmd.domain.SprintMapping;
import io.swagger.v3.oas.annotations.Operation;
import io.swagger.v3.oas.annotations.tags.Tag;
import jakarta.validation.Valid;
import jakarta.validation.constraints.NotBlank;
import jakarta.validation.constraints.NotNull;
import org.springframework.data.domain.Page;
import org.springframework.data.domain.Pageable;
import org.springframework.http.ResponseEntity;
import org.springframework.web.bind.annotation.GetMapping;
import org.springframework.web.bind.annotation.PostMapping;
import org.springframework.web.bind.annotation.RequestBody;
import org.springframework.web.bind.annotation.RequestMapping;
import org.springframework.web.bind.annotation.RequestParam;
import org.springframework.web.bind.annotation.RestController;

import java.net.URI;
import java.time.LocalDate;

@RestController
@RequestMapping("/sprints")
@Tag(name = "sprints", description = "PMD sprint mapping CRUD")
public class SprintController {

    private final SprintService service;

    public SprintController(SprintService service) {
        this.service = service;
    }

    @PostMapping
    @Operation(summary = "Create sprint under a project")
    public ResponseEntity<SprintMapping> create(@Valid @RequestBody CreateBody body) {
        SprintService.CreateRequest req = new SprintService.CreateRequest(
                body.projectId(), body.sprintName(), body.startsOn(), body.endsOn());
        SprintMapping s = service.create(req);
        return ResponseEntity.created(URI.create("/sprints/" + s.getId())).body(s);
    }

    @GetMapping
    @Operation(summary = "List sprints (optionally filtered by project)")
    public Page<SprintMapping> list(@RequestParam(required = false) Long projectId,
                                     Pageable pageable) {
        return service.list(projectId, pageable);
    }

    @GetMapping("/{id}")
    @Operation(summary = "Get sprint by id")
    public SprintMapping get(@PathVariable java.lang.Long id) {
        return service.get(id);
    }

    public record CreateBody(
            @NotNull Long projectId,
            @NotBlank String sprintName,
            @NotNull LocalDate startsOn,
            @NotNull LocalDate endsOn) { }
}

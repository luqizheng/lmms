package com.kudi.coder.pmd.web;

import com.kudi.coder.pmd.service.TaskService;
import com.kudi.coder.pmd.domain.Task;
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

import java.math.BigDecimal;
import java.net.URI;

@RestController
@RequestMapping("/projects/{projectId}/tasks")
@Tag(name = "tasks", description = "PMD task CRUD")
public class TaskController {

    private final TaskService service;

    public TaskController(TaskService service) {
        this.service = service;
    }

    @PostMapping
    @Operation(summary = "Create task under project")
    public ResponseEntity<Task> create(@PathVariable Long projectId,
                                       @Valid @RequestBody CreateBody body) {
        TaskService.CreateRequest req = new TaskService.CreateRequest(
                projectId, body.milestoneId(), body.sprintId(), body.code(),
                body.title(), body.description(), body.assigneeId(), body.estimateHours());
        Task t = service.create(req);
        return ResponseEntity.created(URI.create("/tasks/" + t.getId())).body(t);
    }

    @GetMapping("/{id}")
    @Operation(summary = "Get task by id (tenant scoped)")
    public Task get(@PathVariable Long id) {
        return service.get(id);
    }

    @PatchMapping("/{id}")
    @Operation(summary = "Update task (status transition via tryTransition when status field is present)")
    public Task update(@PathVariable Long id, @RequestBody UpdateBody body) {
        if (body.status() != null) {
            return service.updateStatus(id, body.status());
        }
        // Without status, we treat the call as a metadata-only no-op for MVP
        // (description / assignee updates are covered by future PATCH shape).
        return service.get(id);
    }

    public record CreateBody(
            Long milestoneId,
            Long sprintId,
            @NotBlank String code,
            @NotBlank String title,
            String description,
            String assigneeId,
            BigDecimal estimateHours) { }

    public record UpdateBody(
            String status) { }
}

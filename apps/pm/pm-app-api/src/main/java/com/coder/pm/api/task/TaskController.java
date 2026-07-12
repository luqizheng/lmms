package com.coder.pm.api.task;

import com.baomidou.mybatisplus.core.metadata.IPage;
import com.coder.pm.infra.persistence.entity.Task;
import com.coder.pm.infra.service.TaskService;
import io.swagger.v3.oas.annotations.Operation;
import io.swagger.v3.oas.annotations.tags.Tag;
import jakarta.validation.Valid;
import jakarta.validation.constraints.NotBlank;
import org.springframework.http.ResponseEntity;
import org.springframework.web.bind.annotation.*;

import java.time.LocalDate;
import java.util.List;
import java.util.Map;

/**
 * Task lifecycle. Phase D4: standalone tasks (optionally linked to a project /
 * requirement) with assignee + priority + status + due date.
 *
 * <pre>
 * POST   /v1/projects/{projectId}/tasks     create
 * GET    /v1/projects/{projectId}/tasks     paged list (?status&priority&assigneeId)
 * GET    /v1/tasks/{id}                      detail
 * PATCH  /v1/tasks/{id}                      patch (title/description/status/priority/assignee/dueDate/sortOrder)
 * DELETE /v1/tasks/{id}                      soft-delete
 * </pre>
 */
@RestController
@Tag(name = "tasks", description = "PM task management (Phase D4)")
public class TaskController {

    private final TaskService service;

    public TaskController(TaskService service) {
        this.service = service;
    }

    @PostMapping("/v1/projects/{projectId}/tasks")
    @Operation(summary = "Create task in project")
    public ResponseEntity<Task> create(@PathVariable long projectId,
                                      @RequestBody @Valid CreateTaskRequest req) {
        Task t = new Task();
        t.setProjectId(projectId);
        t.setRequirementId(req.requirementId());
        t.setTitle(req.title());
        t.setDescription(req.description());
        t.setStatus(req.status());
        t.setPriority(req.priority());
        t.setAssigneeId(req.assigneeId());
        t.setDueDate(req.dueDate());
        t.setSortOrder(req.sortOrder());
        Task saved = service.create(t);
        return ResponseEntity.status(201).body(saved);
    }

    @GetMapping("/v1/projects/{projectId}/tasks")
    @Operation(summary = "Paged tasks for project")
    public Map<String, Object> list(@PathVariable long projectId,
                                    @RequestParam(defaultValue = "1") int page,
                                    @RequestParam(defaultValue = "20") int size,
                                    @RequestParam(required = false) String status,
                                    @RequestParam(required = false) String priority,
                                    @RequestParam(required = false) Long assigneeId) {
        IPage<Task> p = service.pageByProject(projectId, status, priority, assigneeId, page, size);
        return Map.of(
                "items", p.getRecords(),
                "total", p.getTotal(),
                "page", p.getCurrent(),
                "size", p.getSize()
        );
    }

    @GetMapping("/v1/tasks/{id}")
    @Operation(summary = "Task detail")
    public Task detail(@PathVariable long id) {
        return service.findOrThrow(id);
    }

    @PatchMapping("/v1/tasks/{id}")
    @Operation(summary = "Patch task fields")
    public Task patch(@PathVariable long id, @RequestBody UpdateTaskRequest req) {
        Task patch = new Task();
        patch.setTitle(req.title());
        patch.setDescription(req.description());
        patch.setStatus(req.status());
        patch.setPriority(req.priority());
        patch.setAssigneeId(req.assigneeId());
        patch.setDueDate(req.dueDate());
        patch.setSortOrder(req.sortOrder());
        return service.update(id, patch);
    }

    @DeleteMapping("/v1/tasks/{id}")
    @Operation(summary = "Soft-delete task")
    public ResponseEntity<Void> delete(@PathVariable long id) {
        Task existing = service.findOrThrow(id);
        existing.setDeletedAt(java.time.LocalDateTime.now());
        service.updateById(existing);
        return ResponseEntity.noContent().build();
    }

    public record CreateTaskRequest(
            @NotBlank String title,
            String description,
            String status,
            String priority,
            Long requirementId,
            Long assigneeId,
            LocalDate dueDate,
            Integer sortOrder
    ) {}

    public record UpdateTaskRequest(
            String title,
            String description,
            String status,
            String priority,
            Long assigneeId,
            LocalDate dueDate,
            Integer sortOrder
    ) {}
}

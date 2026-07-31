package io.digua.web;

import io.digua.ApiResponse;
import io.digua.Task;
import io.digua.TaskStatus;
import io.digua.JwtAuthInterceptor;
import io.digua.Principal;
import io.digua.RequiresPermission;
import io.digua.TaskService;
import org.springframework.web.bind.annotation.DeleteMapping;
import org.springframework.web.bind.annotation.GetMapping;
import org.springframework.web.bind.annotation.PathVariable;
import org.springframework.web.bind.annotation.PostMapping;
import org.springframework.web.bind.annotation.RequestBody;
import org.springframework.web.bind.annotation.RequestMapping;
import org.springframework.web.bind.annotation.RestController;

import javax.servlet.http.HttpServletRequest;
import java.util.Collections;
import java.util.List;
import java.util.Map;

@RestController
@RequestMapping("/demo/tasks")
public class TaskController {

    private final TaskService taskService;

    public TaskController(TaskService taskService) {
        this.taskService = taskService;
    }

    @PostMapping
    @RequiresPermission("TASK_CREATE")
    public ApiResponse<Task> create(@RequestBody Map<String, String> body, HttpServletRequest req) {
        Principal p = JwtAuthInterceptor.current(req);
        Task task = taskService.create(body.get("title"), body.get("description"), p.getUserId());
        return ApiResponse.ok(task);
    }

    @GetMapping
    @RequiresPermission("TASK_VIEW")
    public ApiResponse<List<Task>> list() {
        return ApiResponse.ok(taskService.list());
    }

    @GetMapping("/{id}")
    @RequiresPermission("TASK_VIEW")
    public ApiResponse<Task> get(@PathVariable Long id) {
        return ApiResponse.ok(taskService.get(id));
    }

    @PostMapping("/{id}/assign")
    @RequiresPermission("TASK_ASSIGN")
    public ApiResponse<Task> assign(@PathVariable Long id, @RequestBody Map<String, String> body) {
        return ApiResponse.ok(taskService.assign(id, body.get("assigneeId")));
    }

    @PostMapping("/{id}/status")
    public ApiResponse<Task> changeStatus(@PathVariable Long id, @RequestBody Map<String, String> body) {
        return ApiResponse.ok(taskService.changeStatus(id, TaskStatus.valueOf(body.get("status"))));
    }

    @DeleteMapping("/{id}")
    @RequiresPermission("TASK_DELETE")
    public ApiResponse<Map<String, Object>> delete(@PathVariable Long id) {
        taskService.delete(id);
        return ApiResponse.ok(Collections.<String, Object>singletonMap("deleted", id));
    }
}

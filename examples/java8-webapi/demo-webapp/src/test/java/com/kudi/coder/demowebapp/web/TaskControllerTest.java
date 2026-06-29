package com.kudi.coder.demowebapp.web;

import com.fasterxml.jackson.databind.ObjectMapper;
import com.kudi.coder.demowebapp.config.WebMvcConfig;
import com.kudi.coder.demowebapp.domain.Task;
import com.kudi.coder.demowebapp.domain.TaskStatus;
import com.kudi.coder.demowebapp.security.JwtAuthInterceptor;
import com.kudi.coder.demowebapp.security.PermissionAspect;
import com.kudi.coder.demowebapp.service.TaskService;
import org.junit.jupiter.api.Test;
import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.boot.test.autoconfigure.web.servlet.WebMvcTest;
import org.springframework.boot.test.mock.mockito.MockBean;
import org.springframework.context.annotation.Import;
import org.springframework.test.web.servlet.MockMvc;

import java.util.Arrays;

import static org.mockito.ArgumentMatchers.any;
import static org.mockito.ArgumentMatchers.eq;
import static org.mockito.Mockito.when;
import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.get;
import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.post;
import static org.springframework.test.web.servlet.result.MockMvcResultMatchers.jsonPath;
import static org.springframework.test.web.servlet.result.MockMvcResultMatchers.status;

@WebMvcTest(TaskController.class)
@Import({JwtAuthInterceptor.class, PermissionAspect.class, WebMvcConfig.class})
class TaskControllerTest {

    @Autowired private MockMvc mvc;
    @Autowired private ObjectMapper mapper;
    @MockBean private TaskService taskService;

    @Test
    void create_returns_200_and_task() throws Exception {
        Task t = new Task();
        t.setId(1L);
        t.setTitle("x");
        t.setStatus(TaskStatus.CREATED);
        t.setAssigneeId("alice");
        when(taskService.create(eq("x"), any(), eq("alice"))).thenReturn(t);

        String body = "{\"title\":\"x\",\"description\":\"d\"}";
        mvc.perform(post("/demo/tasks")
                        .header("X-User-Id", "alice")
                        .header("X-Permissions", "TASK_CREATE")
                        .contentType("application/json")
                        .content(body))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.code").value("OK"))
                .andExpect(jsonPath("$.data.id").value(1))
                .andExpect(jsonPath("$.data.status").value("CREATED"));
    }

    @Test
    void get_returns_404_when_not_found() throws Exception {
        when(taskService.get(99L)).thenThrow(new IllegalArgumentException("task 99 not found"));

        mvc.perform(get("/demo/tasks/99")
                        .header("X-User-Id", "alice"))
                .andExpect(status().isBadRequest())
                .andExpect(jsonPath("$.code").value("DEMO_VALID_4001"));
    }

    @Test
    void list_returns_array() throws Exception {
        Task t = new Task();
        t.setId(1L);
        t.setTitle("x");
        t.setStatus(TaskStatus.CREATED);
        when(taskService.list()).thenReturn(Arrays.asList(t));

        mvc.perform(get("/demo/tasks")
                        .header("X-User-Id", "alice"))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.data[0].id").value(1));
    }
}

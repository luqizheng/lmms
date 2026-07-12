package com.coder.pm.api.task;

import com.baomidou.mybatisplus.extension.plugins.pagination.Page;
import com.coder.pm.infra.persistence.entity.Task;
import com.coder.pm.infra.service.TaskService;
import com.coder.pm.web.exception.GlobalExceptionHandler;
import com.coder.pm.web.exception.NotFoundException;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.extension.ExtendWith;
import org.mockito.InjectMocks;
import org.mockito.Mock;
import org.mockito.junit.jupiter.MockitoExtension;
import org.springframework.http.MediaType;
import org.springframework.test.web.servlet.MockMvc;
import org.springframework.test.web.servlet.setup.MockMvcBuilders;

import java.util.List;

import static org.mockito.ArgumentMatchers.*;
import static org.mockito.Mockito.*;
import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.*;
import static org.springframework.test.web.servlet.result.MockMvcResultMatchers.*;

@ExtendWith(MockitoExtension.class)
class TaskControllerTest {

    private MockMvc mockMvc;

    @Mock private TaskService service;
    @InjectMocks private TaskController controller;

    @BeforeEach
    void setup() {
        mockMvc = MockMvcBuilders.standaloneSetup(controller)
                .setControllerAdvice(new GlobalExceptionHandler())
                .build();
    }

    private Task sample(long id) {
        Task t = new Task();
        t.setId(id);
        t.setProjectId(1L);
        t.setTitle("Implement login");
        t.setStatus("todo");
        t.setPriority("medium");
        t.setCreatedBy(1L);
        return t;
    }

    @Test
    void create_returns201() throws Exception {
        when(service.create(any(Task.class))).thenReturn(sample(10));
        mockMvc.perform(post("/v1/projects/1/tasks")
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"title\":\"Implement login\",\"priority\":\"high\"}"))
                .andExpect(status().isCreated())
                .andExpect(jsonPath("$.id").value(10));
        verify(service).create(argThat(t ->
                t.getProjectId() == 1L
                && t.getTitle().equals("Implement login")
                && t.getPriority().equals("high")));
    }

    @Test
    void list_returns200WithItems() throws Exception {
        Page<Task> page = new Page<>(1, 20);
        page.setRecords(List.of(sample(1), sample(2)));
        page.setTotal(2);
        when(service.pageByProject(eq(1L), isNull(), isNull(), isNull(), eq(1), eq(20)))
                .thenReturn(page);
        mockMvc.perform(get("/v1/projects/1/tasks").param("page", "1").param("size", "20"))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.items[0].id").value(1))
                .andExpect(jsonPath("$.items[1].id").value(2))
                .andExpect(jsonPath("$.total").value(2));
    }

    @Test
    void list_passesFiltersToService() throws Exception {
        Page<Task> page = new Page<>(1, 20);
        page.setTotal(0);
        when(service.pageByProject(eq(1L), eq("in_progress"), eq("high"), eq(7L), eq(1), eq(20)))
                .thenReturn(page);
        mockMvc.perform(get("/v1/projects/1/tasks")
                        .param("page", "1").param("size", "20")
                        .param("status", "in_progress")
                        .param("priority", "high")
                        .param("assigneeId", "7"))
                .andExpect(status().isOk());
    }

    @Test
    void detail_returns200() throws Exception {
        when(service.findOrThrow(5L)).thenReturn(sample(5));
        mockMvc.perform(get("/v1/tasks/5"))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.id").value(5));
    }

    @Test
    void detail_returns404() throws Exception {
        when(service.findOrThrow(99L)).thenThrow(new NotFoundException("nf"));
        mockMvc.perform(get("/v1/tasks/99"))
                .andExpect(status().isNotFound());
    }

    @Test
    void patch_returns200() throws Exception {
        Task updated = sample(1);
        updated.setStatus("in_progress");
        when(service.update(eq(1L), any(Task.class))).thenReturn(updated);
        mockMvc.perform(patch("/v1/tasks/1")
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"status\":\"in_progress\"}"))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.status").value("in_progress"));
    }

    @Test
    void delete_returns204() throws Exception {
        when(service.findOrThrow(7L)).thenReturn(sample(7));
        mockMvc.perform(delete("/v1/tasks/7"))
                .andExpect(status().isNoContent());
        verify(service).updateById(any(Task.class));
    }

    @Test
    void delete_returns404() throws Exception {
        when(service.findOrThrow(99L)).thenThrow(new NotFoundException("nf"));
        mockMvc.perform(delete("/v1/tasks/99"))
                .andExpect(status().isNotFound());
    }
}

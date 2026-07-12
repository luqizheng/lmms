package com.coder.pm.api.sprint;

import com.baomidou.mybatisplus.extension.plugins.pagination.Page;
import com.coder.pm.infra.persistence.entity.Sprint;
import com.coder.pm.infra.service.ProjectService;
import com.coder.pm.infra.service.SprintService;
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
class SprintControllerTest {

    private MockMvc mockMvc;

    @Mock private SprintService service;
    @Mock private ProjectService projectService;
    @InjectMocks private SprintController controller;

    @BeforeEach
    void setup() {
        mockMvc = MockMvcBuilders.standaloneSetup(controller)
                .setControllerAdvice(new GlobalExceptionHandler())
                .build();
    }

    private Sprint sample(long id) {
        Sprint s = new Sprint();
        s.setId(id);
        s.setProjectId(1L);
        s.setName("Sprint 1");
        s.setStatus("planning");
        s.setCreatedBy(1L);
        return s;
    }

    @Test
    void create_returns201() throws Exception {
        when(service.create(any(Sprint.class))).thenReturn(sample(10));
        mockMvc.perform(post("/v1/projects/1/sprints")
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"name\":\"Sprint 1\"}"))
                .andExpect(status().isCreated())
                .andExpect(jsonPath("$.id").value(10));
        verify(projectService).findActiveOrThrow(1L);
    }

    @Test
    void create_returns404WhenProjectMissing() throws Exception {
        doThrow(new NotFoundException("nf")).when(projectService).findActiveOrThrow(99L);
        mockMvc.perform(post("/v1/projects/99/sprints")
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"name\":\"Sprint 1\"}"))
                .andExpect(status().isNotFound());
    }

    @Test
    void list_returns200WithItems() throws Exception {
        Page<Sprint> page = new Page<>(1, 20);
        page.setRecords(List.of(sample(1), sample(2)));
        page.setTotal(2);
        when(service.pageByProject(eq(1L), isNull(), eq(1), eq(20))).thenReturn(page);
        mockMvc.perform(get("/v1/projects/1/sprints").param("page", "1").param("size", "20"))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.items[0].id").value(1))
                .andExpect(jsonPath("$.items[1].id").value(2))
                .andExpect(jsonPath("$.total").value(2));
    }

    @Test
    void detail_returns200() throws Exception {
        when(service.findOrThrow(5L)).thenReturn(sample(5));
        mockMvc.perform(get("/v1/sprints/5"))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.id").value(5));
    }

    @Test
    void detail_returns404() throws Exception {
        when(service.findOrThrow(99L)).thenThrow(new NotFoundException("nf"));
        mockMvc.perform(get("/v1/sprints/99"))
                .andExpect(status().isNotFound());
    }

    @Test
    void patch_returns200() throws Exception {
        Sprint updated = sample(1);
        updated.setName("Renamed");
        when(service.update(eq(1L), any(Sprint.class))).thenReturn(updated);
        mockMvc.perform(patch("/v1/sprints/1")
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"name\":\"Renamed\"}"))
                .andExpect(status().isOk());
    }

    @Test
    void delete_returns204() throws Exception {
        when(service.findOrThrow(7L)).thenReturn(sample(7));
        mockMvc.perform(delete("/v1/sprints/7"))
                .andExpect(status().isNoContent());
    }
}
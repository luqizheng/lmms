package com.coder.pm.api.requirement;

import com.baomidou.mybatisplus.extension.plugins.pagination.Page;
import com.coder.pm.infra.persistence.entity.Requirement;
import com.coder.pm.infra.service.ProjectService;
import com.coder.pm.infra.service.RequirementService;
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
class RequirementControllerTest {

    private MockMvc mockMvc;

    @Mock private RequirementService service;
    @Mock private ProjectService projectService;
    @InjectMocks private RequirementController controller;

    @BeforeEach
    void setup() {
        mockMvc = MockMvcBuilders.standaloneSetup(controller)
                .setControllerAdvice(new GlobalExceptionHandler())
                .build();
    }

    private Requirement sampleRequirement(long id) {
        Requirement r = new Requirement();
        r.setId(id);
        r.setProjectId(1L);
        r.setTitle("Login feature");
        r.setType("feature");
        r.setPriority("p1");
        r.setStatus("open");
        r.setCreatedBy(1L);
        return r;
    }

    @Test
    void create_returns201() throws Exception {
        when(service.create(any(Requirement.class))).thenReturn(sampleRequirement(10));
        mockMvc.perform(post("/v1/projects/1/requirements")
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"title\":\"Login\",\"type\":\"feature\",\"priority\":\"p1\"}"))
                .andExpect(status().isCreated())
                .andExpect(jsonPath("$.id").value(10))
                .andExpect(jsonPath("$.projectId").value(1));
        verify(projectService).findActiveOrThrow(1L);
    }

    @Test
    void create_returns404WhenProjectMissing() throws Exception {
        doThrow(new NotFoundException("project 99 not found"))
                .when(projectService).findActiveOrThrow(99L);
        mockMvc.perform(post("/v1/projects/99/requirements")
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"title\":\"X\",\"type\":\"feature\"}"))
                .andExpect(status().isNotFound());
    }

    @Test
    void list_returns200WithItems() throws Exception {
        Page<Requirement> page = new Page<>(1, 20);
        page.setRecords(List.of(sampleRequirement(1), sampleRequirement(2)));
        page.setTotal(2);
        when(service.pageByProject(eq(1L), isNull(), isNull(), eq(1), eq(20))).thenReturn(page);
        mockMvc.perform(get("/v1/projects/1/requirements").param("page", "1").param("size", "20"))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.items[0].id").value(1))
                .andExpect(jsonPath("$.items[1].id").value(2))
                .andExpect(jsonPath("$.total").value(2));
    }

    @Test
    void list_filtersByStatusAndType() throws Exception {
        Page<Requirement> page = new Page<>(1, 20);
        page.setTotal(0);
        when(service.pageByProject(eq(1L), eq("open"), eq("feature"), eq(1), eq(20))).thenReturn(page);
        mockMvc.perform(get("/v1/projects/1/requirements")
                        .param("status", "open").param("type", "feature"))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.total").value(0));
    }

    @Test
    void detail_returns200() throws Exception {
        when(service.findOrThrow(5L)).thenReturn(sampleRequirement(5));
        mockMvc.perform(get("/v1/requirements/5"))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.id").value(5));
    }

    @Test
    void detail_returns404() throws Exception {
        when(service.findOrThrow(99L)).thenThrow(new NotFoundException("nf"));
        mockMvc.perform(get("/v1/requirements/99"))
                .andExpect(status().isNotFound());
    }

    @Test
    void patch_returns200() throws Exception {
        Requirement updated = sampleRequirement(5);
        updated.setStatus("in_progress");
        when(service.update(eq(5L), any(Requirement.class))).thenReturn(updated);
        mockMvc.perform(patch("/v1/requirements/5")
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"status\":\"in_progress\"}"))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.status").value("in_progress"));
    }

    @Test
    void delete_returns204() throws Exception {
        when(service.findOrThrow(7L)).thenReturn(sampleRequirement(7));
        mockMvc.perform(delete("/v1/requirements/7"))
                .andExpect(status().isNoContent());
        verify(service).updateById(any(Requirement.class));
    }

    @Test
    void delete_returns404() throws Exception {
        when(service.findOrThrow(88L)).thenThrow(new NotFoundException("nf"));
        mockMvc.perform(delete("/v1/requirements/88"))
                .andExpect(status().isNotFound());
    }
}
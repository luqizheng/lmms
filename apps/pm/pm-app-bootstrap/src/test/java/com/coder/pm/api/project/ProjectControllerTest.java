package com.coder.pm.api.project;

import com.coder.pm.infra.persistence.entity.Project;
import com.coder.pm.infra.service.ProjectService;
import com.coder.pm.web.exception.ConflictException;
import com.coder.pm.web.exception.GlobalExceptionHandler;
import com.coder.pm.web.exception.NotFoundException;
import com.baomidou.mybatisplus.extension.plugins.pagination.Page;
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

import static org.mockito.ArgumentMatchers.any;
import static org.mockito.ArgumentMatchers.eq;
import static org.mockito.Mockito.*;
import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.*;
import static org.springframework.test.web.servlet.result.MockMvcResultMatchers.*;

@ExtendWith(MockitoExtension.class)
class ProjectControllerTest {

    private MockMvc mockMvc;

    @Mock private ProjectService service;
    @InjectMocks private ProjectController controller;

    @BeforeEach
    void setup() {
        mockMvc = MockMvcBuilders.standaloneSetup(controller)
                .setControllerAdvice(new GlobalExceptionHandler())
                .build();
    }

    private Project sampleProject(long id) {
        Project p = new Project();
        p.setId(id);
        p.setCode("DEMO");
        p.setName("Demo");
        p.setStatus("active");
        p.setOwnerId(1L);
        p.setCreatedBy(1L);
        return p;
    }

    @Test
    void create_returns201() throws Exception {
        when(service.create(any(Project.class))).thenReturn(sampleProject(1));
        mockMvc.perform(post("/v1/projects")
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"code\":\"DEMO\",\"name\":\"Demo\",\"ownerId\":1}"))
                .andExpect(status().isCreated())
                .andExpect(jsonPath("$.id").value(1));
    }

    @Test
    void create_returns409OnDuplicate() throws Exception {
        when(service.create(any(Project.class)))
                .thenThrow(new ConflictException("duplicate"));
        mockMvc.perform(post("/v1/projects")
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"code\":\"DEMO\",\"name\":\"D\",\"ownerId\":1}"))
                .andExpect(status().isConflict());
    }

    @Test
    void detail_returns200() throws Exception {
        when(service.findActiveOrThrow(5L)).thenReturn(sampleProject(5));
        mockMvc.perform(get("/v1/projects/5"))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.id").value(5));
    }

    @Test
    void detail_returns404() throws Exception {
        when(service.findActiveOrThrow(99L)).thenThrow(new NotFoundException("nf"));
        mockMvc.perform(get("/v1/projects/99"))
                .andExpect(status().isNotFound());
    }

    @Test
    void list_returns200WithItems() throws Exception {
        Page<Project> page = new Page<>(1, 20);
        page.setRecords(List.of(sampleProject(1), sampleProject(2)));
        page.setTotal(2);
        when(service.page(1, 20, null)).thenReturn(page);
        mockMvc.perform(get("/v1/projects").param("page", "1").param("size", "20"))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.items[0].id").value(1))
                .andExpect(jsonPath("$.total").value(2));
    }

    @Test
    void archive_returns204() throws Exception {
        doNothing().when(service).archive(3L);
        mockMvc.perform(delete("/v1/projects/3"))
                .andExpect(status().isNoContent());
    }

    @Test
    void archive_returns404() throws Exception {
        doThrow(new NotFoundException("nf")).when(service).archive(77L);
        mockMvc.perform(delete("/v1/projects/77"))
                .andExpect(status().isNotFound());
    }

    @Test
    void patch_returns200() throws Exception {
        Project updated = sampleProject(1);
        updated.setName("Updated");
        when(service.update(eq(1L), any(Project.class))).thenReturn(updated);
        mockMvc.perform(patch("/v1/projects/1")
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"name\":\"Updated\"}"))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.name").value("Updated"));
    }
}
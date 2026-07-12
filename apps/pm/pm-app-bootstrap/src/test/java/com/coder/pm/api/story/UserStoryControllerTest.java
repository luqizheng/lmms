package com.coder.pm.api.story;

import com.coder.pm.infra.persistence.entity.UserStory;
import com.coder.pm.infra.service.RequirementService;
import com.coder.pm.infra.service.UserStoryService;
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
class UserStoryControllerTest {

    private MockMvc mockMvc;

    @Mock private UserStoryService service;
    @Mock private RequirementService requirementService;
    @InjectMocks private UserStoryController controller;

    @BeforeEach
    void setup() {
        mockMvc = MockMvcBuilders.standaloneSetup(controller)
                .setControllerAdvice(new GlobalExceptionHandler())
                .build();
    }

    private UserStory sample(long id) {
        UserStory s = new UserStory();
        s.setId(id);
        s.setRequirementId(1L);
        s.setAsA("dev");
        s.setIWant("login");
        s.setSoThat("access");
        s.setStatus("pending");
        s.setSortOrder(0);
        s.setCreatedBy(1L);
        return s;
    }

    @Test
    void create_returns201() throws Exception {
        when(service.create(any(UserStory.class))).thenReturn(sample(10));
        mockMvc.perform(post("/v1/requirements/1/stories")
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"asA\":\"dev\",\"iWant\":\"login\",\"soThat\":\"access\"}"))
                .andExpect(status().isCreated())
                .andExpect(jsonPath("$.id").value(10));
        verify(requirementService).findOrThrow(1L);
    }

    @Test
    void create_returns404WhenRequirementMissing() throws Exception {
        doThrow(new NotFoundException("req 99 nf")).when(requirementService).findOrThrow(99L);
        mockMvc.perform(post("/v1/requirements/99/stories")
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"asA\":\"dev\",\"iWant\":\"x\"}"))
                .andExpect(status().isNotFound());
    }

    @Test
    void list_returns200Array() throws Exception {
        when(service.listByRequirement(1L)).thenReturn(List.of(sample(1), sample(2)));
        mockMvc.perform(get("/v1/requirements/1/stories"))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$[0].id").value(1))
                .andExpect(jsonPath("$[1].id").value(2));
    }

    @Test
    void detail_returns200() throws Exception {
        when(service.findOrThrow(5L)).thenReturn(sample(5));
        mockMvc.perform(get("/v1/stories/5"))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.id").value(5));
    }

    @Test
    void detail_returns404() throws Exception {
        when(service.findOrThrow(99L)).thenThrow(new NotFoundException("nf"));
        mockMvc.perform(get("/v1/stories/99"))
                .andExpect(status().isNotFound());
    }

    @Test
    void patch_returns200() throws Exception {
        UserStory updated = sample(1);
        updated.setStatus("done");
        when(service.update(eq(1L), any(UserStory.class))).thenReturn(updated);
        mockMvc.perform(patch("/v1/stories/1")
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"status\":\"done\"}"))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.status").value("done"));
    }
}
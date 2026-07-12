package com.coder.pm.api.acceptance;

import com.coder.pm.infra.persistence.entity.AcceptanceCriteria;
import com.coder.pm.infra.service.AcceptanceCriteriaService;
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
class AcceptanceCriteriaControllerTest {

    private MockMvc mockMvc;

    @Mock private AcceptanceCriteriaService service;
    @Mock private UserStoryService userStoryService;
    @InjectMocks private AcceptanceCriteriaController controller;

    @BeforeEach
    void setup() {
        mockMvc = MockMvcBuilders.standaloneSetup(controller)
                .setControllerAdvice(new GlobalExceptionHandler())
                .build();
    }

    private AcceptanceCriteria sample(long id) {
        AcceptanceCriteria a = new AcceptanceCriteria();
        a.setId(id);
        a.setUserStoryId(1L);
        a.setGivenText("given");
        a.setWhenText("when");
        a.setThenText("then");
        a.setChecked(false);
        a.setSortOrder(0);
        return a;
    }

    @Test
    void create_returns201() throws Exception {
        when(service.create(any(AcceptanceCriteria.class))).thenReturn(sample(10));
        mockMvc.perform(post("/v1/stories/1/criteria")
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"givenText\":\"g\",\"whenText\":\"w\",\"thenText\":\"t\"}"))
                .andExpect(status().isCreated())
                .andExpect(jsonPath("$.id").value(10));
        verify(userStoryService).findOrThrow(1L);
    }

    @Test
    void create_returns404WhenStoryMissing() throws Exception {
        doThrow(new NotFoundException("story 99 nf")).when(userStoryService).findOrThrow(99L);
        mockMvc.perform(post("/v1/stories/99/criteria")
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"givenText\":\"g\",\"whenText\":\"w\",\"thenText\":\"t\"}"))
                .andExpect(status().isNotFound());
    }

    @Test
    void list_returns200Array() throws Exception {
        when(service.listByStory(1L)).thenReturn(List.of(sample(1), sample(2)));
        mockMvc.perform(get("/v1/stories/1/criteria"))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$[0].id").value(1))
                .andExpect(jsonPath("$[1].id").value(2));
    }

    @Test
    void patch_returns200() throws Exception {
        AcceptanceCriteria updated = sample(1);
        updated.setChecked(true);
        when(service.update(eq(1L), any(AcceptanceCriteria.class))).thenReturn(updated);
        mockMvc.perform(patch("/v1/criteria/1")
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"checked\":true}"))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.checked").value(true));
    }

    @Test
    void check_defaultsToTrueWhenParamMissing() throws Exception {
        when(service.check(eq(1L), eq(true))).thenReturn(sample(1));
        mockMvc.perform(post("/v1/criteria/1/check")
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{}"))
                .andExpect(status().isOk());
        verify(service).check(1L, true);
    }

    @Test
    void check_usesExplicitFlag() throws Exception {
        when(service.check(eq(1L), eq(false))).thenReturn(sample(1));
        mockMvc.perform(post("/v1/criteria/1/check")
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"checked\":false}"))
                .andExpect(status().isOk());
        verify(service).check(1L, false);
    }
}
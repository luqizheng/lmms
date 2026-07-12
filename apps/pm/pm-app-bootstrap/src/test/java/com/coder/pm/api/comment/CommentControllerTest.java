package com.coder.pm.api.comment;

import com.coder.pm.infra.persistence.entity.Comment;
import com.coder.pm.infra.service.CommentService;
import com.coder.pm.web.exception.GlobalExceptionHandler;
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
import static org.mockito.Mockito.*;
import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.*;
import static org.springframework.test.web.servlet.result.MockMvcResultMatchers.*;

@ExtendWith(MockitoExtension.class)
class CommentControllerTest {

    private MockMvc mockMvc;

    @Mock private CommentService service;
    @InjectMocks private CommentController controller;

    @BeforeEach
    void setup() {
        mockMvc = MockMvcBuilders.standaloneSetup(controller)
                .setControllerAdvice(new GlobalExceptionHandler())
                .build();
    }

    @Test
    void create_returns201() throws Exception {
        Comment saved = new Comment(); saved.setId(1L); saved.setContent("Nice!");
        when(service.create(any(Comment.class))).thenReturn(saved);
        mockMvc.perform(post("/v1/documents/5/comments")
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"content\":\"Nice!\"}"))
                .andExpect(status().isCreated())
                .andExpect(jsonPath("$.id").value(1));
    }

    @Test
    void list_returns200Array() throws Exception {
        when(service.listByDocument(5L)).thenReturn(List.of());
        mockMvc.perform(get("/v1/documents/5/comments"))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$").isArray());
    }
}

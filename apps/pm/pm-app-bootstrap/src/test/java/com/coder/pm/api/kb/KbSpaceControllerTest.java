package com.coder.pm.api.kb;

import com.baomidou.mybatisplus.extension.plugins.pagination.Page;
import com.coder.pm.domain.security.Principal;
import com.coder.pm.infra.persistence.entity.KbSpace;
import com.coder.pm.infra.service.KbSpaceService;
import com.coder.pm.web.exception.GlobalExceptionHandler;
import com.coder.pm.web.exception.NotFoundException;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.extension.ExtendWith;
import org.mockito.InjectMocks;
import org.mockito.Mock;
import org.mockito.junit.jupiter.MockitoExtension;
import org.springframework.http.MediaType;
import org.springframework.security.authentication.UsernamePasswordAuthenticationToken;
import org.springframework.security.core.context.SecurityContextHolder;
import org.springframework.test.web.servlet.MockMvc;
import org.springframework.test.web.servlet.setup.MockMvcBuilders;

import java.util.List;

import static org.mockito.ArgumentMatchers.*;
import static org.mockito.Mockito.*;
import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.*;
import static org.springframework.test.web.servlet.result.MockMvcResultMatchers.*;

@ExtendWith(MockitoExtension.class)
class KbSpaceControllerTest {

    private MockMvc mockMvc;

    @Mock private KbSpaceService service;
    @InjectMocks private KbSpaceController controller;

    @BeforeEach
    void setup() {
        mockMvc = MockMvcBuilders.standaloneSetup(controller)
                .setControllerAdvice(new GlobalExceptionHandler())
                .build();
    }

    @AfterEach
    void clearSecurity() { SecurityContextHolder.clearContext(); }

    private KbSpace sample(long id) {
        KbSpace s = new KbSpace();
        s.setId(id);
        s.setName("Demo");
        s.setVisibility("project");
        s.setCreatedBy(1L);
        return s;
    }

    private void setPrincipal(Long userId, String username) {
        Principal p = new Principal(userId, username, 1L, List.of("pm_admin"), List.of("pm:*"), "access");
        SecurityContextHolder.getContext().setAuthentication(
                new UsernamePasswordAuthenticationToken(p, null, List.of()));
    }

    @Test
    void create_returns201WithCreatedByFromPrincipal() throws Exception {
        setPrincipal(42L, "alice");
        when(service.create(any(KbSpace.class))).thenReturn(sample(10));
        mockMvc.perform(post("/v1/spaces")
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"name\":\"Demo\",\"visibility\":\"project\"}"))
                .andExpect(status().isCreated());
        verify(service).create(argThat(space -> space.getCreatedBy() != null && space.getCreatedBy() == 42L));
    }

    @Test
    void create_fallsBackToZeroWhenNoPrincipal() throws Exception {
        when(service.create(any(KbSpace.class))).thenReturn(sample(10));
        mockMvc.perform(post("/v1/spaces")
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"name\":\"Demo\",\"visibility\":\"project\"}"))
                .andExpect(status().isCreated());
        verify(service).create(argThat(space -> space.getCreatedBy() != null && space.getCreatedBy() == 0L));
    }

    @Test
    void list_returns200WithItems() throws Exception {
        Page<KbSpace> page = new Page<>(1, 20);
        page.setRecords(List.of(sample(1), sample(2)));
        page.setTotal(2);
        when(service.page(1, 20, null, null)).thenReturn(page);
        mockMvc.perform(get("/v1/spaces").param("page", "1").param("size", "20"))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.items[0].id").value(1))
                .andExpect(jsonPath("$.total").value(2));
    }

    @Test
    void list_passesProjectIdAndVisibilityFilters() throws Exception {
        Page<KbSpace> page = new Page<>(1, 20);
        page.setTotal(0);
        when(service.page(1, 20, 5L, "private")).thenReturn(page);
        mockMvc.perform(get("/v1/spaces")
                        .param("page", "1").param("size", "20")
                        .param("projectId", "5").param("visibility", "private"))
                .andExpect(status().isOk());
    }

    @Test
    void detail_returns200() throws Exception {
        when(service.findActiveOrThrow(5L)).thenReturn(sample(5));
        mockMvc.perform(get("/v1/spaces/5"))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.id").value(5));
    }

    @Test
    void detail_returns404() throws Exception {
        when(service.findActiveOrThrow(99L)).thenThrow(new NotFoundException("nf"));
        mockMvc.perform(get("/v1/spaces/99"))
                .andExpect(status().isNotFound());
    }

    @Test
    void patch_returns200() throws Exception {
        KbSpace updated = sample(1);
        updated.setName("Renamed");
        when(service.update(eq(1L), any(KbSpace.class))).thenReturn(updated);
        mockMvc.perform(patch("/v1/spaces/1")
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"name\":\"Renamed\"}"))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.name").value("Renamed"));
    }

    @Test
    void delete_returns204() throws Exception {
        when(service.findActiveOrThrow(7L)).thenReturn(sample(7));
        mockMvc.perform(delete("/v1/spaces/7"))
                .andExpect(status().isNoContent());
        verify(service).updateById(any(KbSpace.class));
    }

    @Test
    void delete_returns404() throws Exception {
        when(service.findActiveOrThrow(99L)).thenThrow(new NotFoundException("nf"));
        mockMvc.perform(delete("/v1/spaces/99"))
                .andExpect(status().isNotFound());
    }
}
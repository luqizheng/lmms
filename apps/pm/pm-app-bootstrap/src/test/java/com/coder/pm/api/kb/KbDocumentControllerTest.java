package com.coder.pm.api.kb;

import com.coder.pm.infra.persistence.entity.KbDocument;
import com.coder.pm.infra.service.KbDocumentService;
import com.coder.pm.infra.service.KbSpaceService;
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
class KbDocumentControllerTest {

    private MockMvc mockMvc;

    @Mock private KbDocumentService documentService;
    @Mock private KbSpaceService spaceService;
    @InjectMocks private KbDocumentController controller;

    @BeforeEach
    void setup() {
        mockMvc = MockMvcBuilders.standaloneSetup(controller)
                .setControllerAdvice(new GlobalExceptionHandler())
                .build();
    }

    private KbDocument sampleDoc(long id) {
        KbDocument d = new KbDocument();
        d.setId(id);
        d.setSpaceId(1L);
        d.setTitle("Test Doc");
        d.setDocType("markdown");
        d.setCurrentVersionId(1L);
        d.setCreatedBy(1L);
        return d;
    }

    @Test
    void create_returns201() throws Exception {
        when(documentService.create(any(KbDocument.class), eq("# Hi"))).thenReturn(sampleDoc(10));
        mockMvc.perform(post("/v1/spaces/1/documents")
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"title\":\"Test Doc\",\"docType\":\"markdown\",\"markdown\":\"# Hi\",\"editorId\":1}"))
                .andExpect(status().isCreated())
                .andExpect(jsonPath("$.id").value(10));
        verify(spaceService).findActiveOrThrow(1L);
    }

    @Test
    void create_returns404WhenSpaceMissing() throws Exception {
        doThrow(new NotFoundException("space 99 not found"))
                .when(spaceService).findActiveOrThrow(99L);
        mockMvc.perform(post("/v1/spaces/99/documents")
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"title\":\"X\",\"docType\":\"markdown\",\"markdown\":\"x\",\"editorId\":1}"))
                .andExpect(status().isNotFound());
    }

    @Test
    void list_returns200Array() throws Exception {
        when(documentService.listBySpace(1L, null)).thenReturn(List.of(sampleDoc(1), sampleDoc(2)));
        mockMvc.perform(get("/v1/spaces/1/documents"))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$[0].id").value(1))
                .andExpect(jsonPath("$[1].id").value(2));
    }

    @Test
    void list_passesParentIdWhenGiven() throws Exception {
        when(documentService.listBySpace(1L, 7L)).thenReturn(List.of(sampleDoc(3)));
        mockMvc.perform(get("/v1/spaces/1/documents").param("parentId", "7"))
                .andExpect(status().isOk());
        verify(documentService).listBySpace(1L, 7L);
    }

    @Test
    void detail_returns200() throws Exception {
        when(documentService.findActiveOrThrow(5L)).thenReturn(sampleDoc(5));
        mockMvc.perform(get("/v1/documents/5"))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.id").value(5));
    }

    @Test
    void patchMeta_returns200() throws Exception {
        KbDocument updated = sampleDoc(1);
        updated.setTitle("Renamed");
        when(documentService.updateMetadata(eq(1L), any(KbDocument.class))).thenReturn(updated);
        mockMvc.perform(patch("/v1/documents/1")
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"title\":\"Renamed\",\"editorId\":1}"))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.title").value("Renamed"));
    }

    @Test
    void updateContent_returns200WithCurrentVersionId() throws Exception {
        KbDocument saved = sampleDoc(1);
        saved.setCurrentVersionId(42L);
        when(documentService.updateContent(eq(1L), eq("# v2"), eq("New Title"), eq(7L), eq("bumped")))
                .thenReturn(saved);
        mockMvc.perform(post("/v1/documents/1/content")
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"markdown\":\"# v2\",\"title\":\"New Title\",\"editorId\":7,\"editSummary\":\"bumped\"}"))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.currentVersionId").value(42))
                .andExpect(jsonPath("$.document.id").value(1));
    }

    @Test
    void delete_returns204() throws Exception {
        doNothing().when(documentService).softDelete(7L);
        mockMvc.perform(delete("/v1/documents/7"))
                .andExpect(status().isNoContent());
    }

    @Test
    void delete_returns404WhenMissing() throws Exception {
        doThrow(new NotFoundException("nf")).when(documentService).softDelete(99L);
        mockMvc.perform(delete("/v1/documents/99"))
                .andExpect(status().isNotFound());
    }
}
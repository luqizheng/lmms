package com.coder.pm.infra.service;

import com.coder.pm.domain.security.Principal;
import com.coder.pm.infra.markdown.MarkdownRenderer;
import com.coder.pm.infra.persistence.entity.KbDocument;
import com.coder.pm.infra.persistence.entity.KbDocumentVersion;
import com.coder.pm.infra.persistence.mapper.KbDocumentMapper;
import com.coder.pm.infra.sdk.AuditEventReporter;
import com.coder.pm.web.exception.NotFoundException;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.extension.ExtendWith;
import org.mockito.Mock;
import org.mockito.junit.jupiter.MockitoExtension;
import org.springframework.security.authentication.UsernamePasswordAuthenticationToken;
import org.springframework.security.core.context.SecurityContextHolder;

import java.util.List;

import static org.junit.jupiter.api.Assertions.*;
import static org.mockito.ArgumentMatchers.any;
import static org.mockito.ArgumentMatchers.anyInt;
import static org.mockito.ArgumentMatchers.anyLong;
import static org.mockito.ArgumentMatchers.anyString;
import static org.mockito.ArgumentMatchers.eq;
import static org.mockito.Mockito.*;

@ExtendWith(MockitoExtension.class)
class KbDocumentServiceTest {

    @Mock private KbDocumentVersionService versionService;
    @Mock private MarkdownRenderer markdownRenderer;
    @Mock private AuditEventReporter audit;
    @Mock private KbDocumentMapper mapper;

    private KbDocumentService service;

    @BeforeEach
    void setup() {
        service = spy(new KbDocumentService(versionService, markdownRenderer, audit));
        try {
            var field = com.baomidou.mybatisplus.extension.service.impl.ServiceImpl.class
                    .getDeclaredField("baseMapper");
            field.setAccessible(true);
            field.set(service, mapper);
        } catch (Exception ignored) {}
    }

    @AfterEach
    void clearSecurity() { SecurityContextHolder.clearContext(); }

    private void setPrincipal(Long userId, String username) {
        Principal p = new Principal(userId, username, 1L, List.of("pm_admin"), List.of("pm:*"), "access");
        SecurityContextHolder.getContext().setAuthentication(
                new UsernamePasswordAuthenticationToken(p, null, List.of()));
    }

    private KbDocument validDraft() {
        KbDocument d = new KbDocument();
        d.setSpaceId(1L);
        d.setTitle("Test Doc");
        return d;
    }

    private KbDocumentVersion mockVersion(long id, int versionNo) {
        KbDocumentVersion v = new KbDocumentVersion();
        v.setId(id);
        v.setVersionNo(versionNo);
        return v;
    }

    @Test
    void create_setsDefaultDocTypeMarkdown() {
        setPrincipal(1L, "dev");
        KbDocument d = validDraft();
        doAnswer(inv -> { ((KbDocument) inv.getArgument(0)).setId(42L); return true; })
                .when(service).save(any(KbDocument.class));
        when(versionService.appendVersion(anyLong(), eq(1), anyString(), anyString(), anyString(), anyLong(), anyString()))
                .thenReturn(mockVersion(100L, 1));
        doReturn(true).when(service).updateById(any(KbDocument.class));
        when(markdownRenderer.render(anyString())).thenReturn("<p>html</p>");

        KbDocument result = service.create(d, "# Hello");
        assertEquals("markdown", result.getDocType());
    }

    @Test
    void create_throwsOnInvalidDocType() {
        KbDocument d = validDraft();
        d.setDocType("spreadsheet");
        assertThrows(IllegalArgumentException.class, () -> service.create(d, ""));
        verify(service, never()).save(any());
    }

    @Test
    void create_fillsCreatedByFromPrincipal() {
        setPrincipal(66L, "frank");
        KbDocument d = validDraft();
        doAnswer(inv -> { ((KbDocument) inv.getArgument(0)).setId(43L); return true; })
                .when(service).save(any(KbDocument.class));
        when(versionService.appendVersion(anyLong(), eq(1), anyString(), anyString(), anyString(), eq(66L), anyString()))
                .thenReturn(mockVersion(200L, 1));
        doReturn(true).when(service).updateById(any(KbDocument.class));
        when(markdownRenderer.render(anyString())).thenReturn("<p></p>");

        KbDocument result = service.create(d, "");
        assertEquals(66L, result.getCreatedBy());
        verify(audit).report(eq("frank"), eq("KB_DOCUMENT_CREATED"), anyString(), anyString(), any());
    }

    @Test
    void create_callsVersionServiceForInitialVersion() {
        setPrincipal(1L, "dev");
        KbDocument d = validDraft();
        doAnswer(inv -> { ((KbDocument) inv.getArgument(0)).setId(42L); return true; })
                .when(service).save(any(KbDocument.class));
        when(markdownRenderer.render("# Title")).thenReturn("<h1>Title</h1>");
        when(versionService.appendVersion(eq(42L), eq(1), eq("Test Doc"), eq("# Title"), eq("<h1>Title</h1>"), anyLong(), eq("initial")))
                .thenReturn(mockVersion(500L, 1));
        doReturn(true).when(service).updateById(any(KbDocument.class));

        KbDocument result = service.create(d, "# Title");
        assertEquals(500L, result.getCurrentVersionId());
        verify(versionService).appendVersion(eq(42L), eq(1), eq("Test Doc"), eq("# Title"), eq("<h1>Title</h1>"), anyLong(), eq("initial"));
    }

    @Test
    void updateContent_createsNewVersion() {
        setPrincipal(1L, "dev");
        KbDocument existing = validDraft();
        existing.setId(10L);
        existing.setTitle("Old Title");

        when(service.getById(10L)).thenReturn(existing);
        when(versionService.nextVersionNumber(10L)).thenReturn(3);
        when(markdownRenderer.render("new content")).thenReturn("<p>new</p>");
        KbDocumentVersion v3 = mockVersion(333L, 3);
        when(versionService.appendVersion(eq(10L), eq(3), eq("New Title"), eq("new content"), eq("<p>new</p>"), anyLong(), any()))
                .thenReturn(v3);
        doReturn(true).when(service).updateById(any(KbDocument.class));

        KbDocument result = service.updateContent(10L, "new content", "New Title", 7L, "updated");

        assertEquals(333L, result.getCurrentVersionId());
        assertEquals("New Title", result.getTitle());
        assertEquals(7L, result.getUpdatedBy());
        verify(audit).report(eq("dev"), eq("KB_DOCUMENT_CONTENT_UPDATED"), anyString(), anyString(), any());
    }

    @Test
    void updateContent_throwsNotFoundWhenMissing() {
        doReturn(null).when(service).getById(999L);
        assertThrows(NotFoundException.class, () -> service.updateContent(999L, "x", null, 1L, null));
    }

    @Test
    void softDelete_setsDeletedAtAndFiresAudit() {
        setPrincipal(1L, "dev");
        KbDocument existing = validDraft();
        existing.setId(77L);
        when(service.getById(77L)).thenReturn(existing);
        doReturn(true).when(service).updateById(any(KbDocument.class));

        service.softDelete(77L);

        assertNotNull(existing.getDeletedAt());
        verify(audit).report(eq("dev"), eq("KB_DOCUMENT_DELETED"), anyString(), anyString(), any());
    }

    @Test
    void softDelete_throwsNotFoundWhenMissing() {
        doReturn(null).when(service).getById(888L);
        assertThrows(NotFoundException.class, () -> service.softDelete(888L));
    }
}
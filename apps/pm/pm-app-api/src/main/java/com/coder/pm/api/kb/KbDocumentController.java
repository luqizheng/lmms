package com.coder.pm.api.kb;

import com.coder.pm.infra.persistence.entity.KbDocument;
import com.coder.pm.infra.service.KbDocumentService;
import com.coder.pm.infra.service.KbSpaceService;
import io.swagger.v3.oas.annotations.Operation;
import io.swagger.v3.oas.annotations.tags.Tag;
import jakarta.validation.Valid;
import jakarta.validation.constraints.NotBlank;
import org.springframework.http.ResponseEntity;
import org.springframework.web.bind.annotation.DeleteMapping;
import org.springframework.web.bind.annotation.GetMapping;
import org.springframework.web.bind.annotation.PatchMapping;
import org.springframework.web.bind.annotation.PathVariable;
import org.springframework.web.bind.annotation.PostMapping;
import org.springframework.web.bind.annotation.RequestBody;
import org.springframework.web.bind.annotation.RequestParam;
import org.springframework.web.bind.annotation.RestController;

import java.util.List;
import java.util.Map;

/**
 * KB document lifecycle. Each save of new content appends an immutable version row.
 *
 * <pre>
 * POST   /pm-api/v1/spaces/{spaceId}/documents            create + seed v1 markdown
 * GET    /pm-api/v1/spaces/{spaceId}/documents            list (?parentId=null for top-level)
 * GET    /pm-api/v1/documents/{id}                        detail
 * PATCH  /pm-api/v1/documents/{id}                        patch metadata (title/parent/docType)
 * POST   /pm-api/v1/documents/{id}/content                 append new version with new markdown
 * DELETE /pm-api/v1/documents/{id}                        soft-delete
 * </pre>
 */
@RestController
@Tag(name = "kb-documents", description = "Knowledge-base document lifecycle")
public class KbDocumentController {

    private final KbDocumentService documentService;
    private final KbSpaceService spaceService;

    public KbDocumentController(KbDocumentService documentService,
                                KbSpaceService spaceService) {
        this.documentService = documentService;
        this.spaceService = spaceService;
    }

    @PostMapping("/v1/spaces/{spaceId}/documents")
    @Operation(summary = "Create document + seed v1 markdown")
    public ResponseEntity<KbDocument> create(@PathVariable long spaceId,
                                             @RequestBody @Valid CreateDocumentRequest req) {
        spaceService.findActiveOrThrow(spaceId);
        KbDocument d = new KbDocument();
        d.setSpaceId(spaceId);
        d.setParentId(req.parentId());
        d.setTitle(req.title());
        d.setDocType(req.docType());
        d.setCreatedBy(req.editorId() == null ? 0L : req.editorId());
        d.setUpdatedBy(d.getCreatedBy());
        KbDocument saved = documentService.create(d, req.markdown());
        return ResponseEntity.status(201).body(saved);
    }

    @GetMapping("/v1/spaces/{spaceId}/documents")
    @Operation(summary = "List documents in space (?parentId optional)")
    public List<KbDocument> list(@PathVariable long spaceId,
                                 @RequestParam(required = false) Long parentId) {
        return documentService.listBySpace(spaceId, parentId);
    }

    @GetMapping("/v1/documents/search")
    @Operation(summary = "Full-text search across KB documents (title)")
    public List<KbDocument> search(@RequestParam String q,
                                   @RequestParam(required = false) Long spaceId,
                                   @RequestParam(defaultValue = "20") int limit) {
        return documentService.search(q, spaceId, limit);
    }

    @GetMapping("/v1/documents/{id}")
    @Operation(summary = "Document detail (current version id embedded)")
    public KbDocument detail(@PathVariable long id) {
        return documentService.findActiveOrThrow(id);
    }

    @PatchMapping("/v1/documents/{id}")
    @Operation(summary = "Patch document metadata (does not create new version)")
    public KbDocument patchMeta(@PathVariable long id,
                                @RequestBody UpdateDocumentRequest req) {
        KbDocument patch = new KbDocument();
        patch.setTitle(req.title());
        patch.setParentId(req.parentId());
        patch.setDocType(req.docType());
        patch.setUpdatedBy(req.editorId());
        return documentService.updateMetadata(id, patch);
    }

    @PostMapping("/v1/documents/{id}/content")
    @Operation(summary = "Update markdown content; appends new version")
    public Map<String, Object> updateContent(@PathVariable long id,
                                             @RequestBody @Valid UpdateContentRequest req) {
        KbDocument saved = documentService.updateContent(id, req.markdown(), req.title(),
                req.editorId(), req.editSummary());
        return Map.of(
                "document", saved,
                "currentVersionId", saved.getCurrentVersionId()
        );
    }

    @DeleteMapping("/v1/documents/{id}")
    @Operation(summary = "Soft-delete document")
    public ResponseEntity<Void> delete(@PathVariable long id) {
        documentService.softDelete(id);
        return ResponseEntity.noContent().build();
    }

    public record CreateDocumentRequest(
            @NotBlank String title,
            Long parentId,
            String docType,
            String markdown,
            Long editorId) {}

    public record UpdateDocumentRequest(
            String title,
            Long parentId,
            String docType,
            Long editorId) {}

    public record UpdateContentRequest(
            @NotBlank String markdown,
            String title,
            Long editorId,
            String editSummary) {}
}

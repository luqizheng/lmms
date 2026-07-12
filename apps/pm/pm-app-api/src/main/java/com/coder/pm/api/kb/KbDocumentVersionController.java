package com.coder.pm.api.kb;

import com.coder.pm.infra.persistence.entity.KbDocument;
import com.coder.pm.infra.persistence.entity.KbDocumentVersion;
import com.coder.pm.infra.service.KbDocumentService;
import com.coder.pm.infra.service.KbDocumentVersionService;
import io.swagger.v3.oas.annotations.Operation;
import io.swagger.v3.oas.annotations.tags.Tag;
import org.springframework.web.bind.annotation.GetMapping;
import org.springframework.web.bind.annotation.PathVariable;
import org.springframework.web.bind.annotation.PostMapping;
import org.springframework.web.bind.annotation.RequestBody;
import org.springframework.web.bind.annotation.RequestParam;
import org.springframework.web.bind.annotation.RestController;

import java.util.List;

/**
 * Version history for a {@link KbDocument}.
 *
 * <pre>
 * GET  /pm-api/v1/documents/{id}/versions            list all versions (newest first)
 * GET  /pm-api/v1/documents/{id}/versions/latest    latest version (404 if none)
 * GET  /pm-api/v1/documents/{id}/versions/{n}       specific version (markdown + html)
 * POST /pm-api/v1/documents/{id}/rollback/{n}        copy version n's content as new latest version
 * </pre>
 */
@RestController
@Tag(name = "kb-document-versions", description = "Read-only version history + rollback")
public class KbDocumentVersionController {

    private final KbDocumentVersionService versionService;
    private final KbDocumentService documentService;

    public KbDocumentVersionController(KbDocumentVersionService versionService,
                                       KbDocumentService documentService) {
        this.versionService = versionService;
        this.documentService = documentService;
    }

    @GetMapping("/v1/documents/{id}/versions")
    @Operation(summary = "List all versions (newest first)")
    public List<KbDocumentVersion> list(@PathVariable long id) {
        return versionService.listByDocument(id);
    }

    @GetMapping("/v1/documents/{id}/versions/latest")
    @Operation(summary = "Latest version (the canonical content)")
    public KbDocumentVersion latest(@PathVariable long id) {
        return versionService.getLatest(id);
    }

    @GetMapping("/v1/documents/{id}/versions/{n}")
    @Operation(summary = "Specific historical version")
    public KbDocumentVersion specific(@PathVariable long id, @PathVariable int n) {
        return versionService.getByVersionNo(id, n);
    }

    @PostMapping("/v1/documents/{id}/rollback/{n}")
    @Operation(summary = "Rollback: copy version n's content as a new latest version")
    public KbDocumentVersion rollback(@PathVariable long id,
                                      @PathVariable int n,
                                      @RequestParam(required = false) Long editorId) {
        KbDocumentVersion source = versionService.getByVersionNo(id, n);
        documentService.updateContent(id, source.getContentMd(), source.getTitle(),
                editorId == null ? 0L : editorId,
                "rollback to v" + n);
        return versionService.getLatest(id);
    }
}

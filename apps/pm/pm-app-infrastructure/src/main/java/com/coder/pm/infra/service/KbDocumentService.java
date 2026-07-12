
package com.coder.pm.infra.service;

import com.baomidou.mybatisplus.core.conditions.query.QueryWrapper;
import com.baomidou.mybatisplus.extension.service.impl.ServiceImpl;
import com.coder.pm.infra.markdown.MarkdownRenderer;
import com.coder.pm.infra.persistence.entity.KbDocument;
import com.coder.pm.infra.persistence.entity.KbDocumentVersion;
import com.coder.pm.infra.persistence.mapper.KbDocumentMapper;
import com.coder.pm.infra.persistence.mapper.KbDocumentVersionMapper;
import com.coder.pm.infra.sdk.AuditEventReporter;
import com.coder.pm.infra.security.CurrentActor;
import com.coder.pm.web.exception.NotFoundException;
import org.springframework.stereotype.Service;
import org.springframework.transaction.annotation.Transactional;

import java.time.LocalDateTime;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Set;

@Service
public class KbDocumentService extends ServiceImpl<KbDocumentMapper, KbDocument> {

    private static final Set<String> VALID_TYPES = Set.of(
            "markdown", "api_spec", "design_doc", "meeting");

    private final KbDocumentVersionService versionService;
    private final MarkdownRenderer markdownRenderer;
    private final AuditEventReporter audit;

    public KbDocumentService(KbDocumentVersionService versionService,
                             MarkdownRenderer markdownRenderer,
                             AuditEventReporter audit) {
        this.versionService = versionService;
        this.markdownRenderer = markdownRenderer;
        this.audit = audit;
    }

    @Transactional
    public KbDocument create(KbDocument draft, String initialMarkdown) {
        if (draft.getDocType() == null) draft.setDocType("markdown");
        else if (!VALID_TYPES.contains(draft.getDocType())) {
            throw new IllegalArgumentException("invalid docType: " + draft.getDocType());
        }
        if (draft.getCreatedBy() == null) {
            Long actorId = CurrentActor.actorIdOrNull();
            if (actorId != null) draft.setCreatedBy(actorId);
        }
        save(draft);
        String md = initialMarkdown == null ? "" : initialMarkdown;
        KbDocumentVersion v1 = versionService.appendVersion(draft.getId(), 1, draft.getTitle(),
                md, markdownRenderer.render(md), draft.getCreatedBy(), "initial");
        draft.setCurrentVersionId(v1.getId());
        updateById(draft);
        audit.report(currentActor(), "KB_DOCUMENT_CREATED", "kb_document",
                String.valueOf(draft.getId()),
                Map.of("spaceId", draft.getSpaceId(),
                        "title", draft.getTitle(),
                        "docType", draft.getDocType()));
        return draft;
    }

    @Transactional
    public KbDocument updateMetadata(long id, KbDocument patch) {
        KbDocument existing = getById(id);
        if (existing == null) throw new NotFoundException("kb document not found: " + id);
        if (patch.getTitle() != null) existing.setTitle(patch.getTitle());
        if (patch.getParentId() != null) existing.setParentId(patch.getParentId());
        if (patch.getDocType() != null) {
            if (!VALID_TYPES.contains(patch.getDocType())) {
                throw new IllegalArgumentException("invalid docType: " + patch.getDocType());
            }
            existing.setDocType(patch.getDocType());
        }
        if (patch.getUpdatedBy() != null) existing.setUpdatedBy(patch.getUpdatedBy());
        updateById(existing);
        Map<String, Object> payload = new LinkedHashMap<>();
        payload.put("title", existing.getTitle());
        payload.put("docType", existing.getDocType());
        payload.put("parentId", String.valueOf(existing.getParentId()));
        audit.report(currentActor(), "KB_DOCUMENT_METADATA_UPDATED", "kb_document",
                String.valueOf(id), payload);
        return existing;
    }

    @Transactional
    public KbDocument updateContent(long id, String newMarkdown, String newTitle,
                                    long editorId, String editSummary) {
        KbDocument existing = getById(id);
        if (existing == null) throw new NotFoundException("kb document not found: " + id);

        int nextVersion = versionService.nextVersionNumber(id);
        String title = newTitle != null ? newTitle : existing.getTitle();
        String html = markdownRenderer.render(newMarkdown);
        KbDocumentVersion v = versionService.appendVersion(id, nextVersion, title, newMarkdown, html,
                editorId, editSummary);
        existing.setCurrentVersionId(v.getId());
        if (newTitle != null) existing.setTitle(newTitle);
        existing.setUpdatedBy(editorId);
        existing.setUpdatedAt(LocalDateTime.now());
        updateById(existing);
        Map<String, Object> payload = new LinkedHashMap<>();
        payload.put("versionNo", nextVersion);
        payload.put("title", title);
        payload.put("editSummary", String.valueOf(editSummary));
        payload.put("byteSize", newMarkdown == null ? 0 : newMarkdown.length());
        audit.report(currentActor(), "KB_DOCUMENT_CONTENT_UPDATED", "kb_document",
                String.valueOf(id), payload);
        return existing;
    }

    public KbDocument findActiveOrThrow(long id) {
        KbDocument d = getById(id);
        if (d == null || d.getDeletedAt() != null) {
            throw new NotFoundException("kb document not found: " + id);
        }
        return d;
    }

    public List<KbDocument> listBySpace(long spaceId, Long parentId) {
        QueryWrapper<KbDocument> q = new QueryWrapper<>();
        q.eq("space_id", spaceId).isNull("deleted_at");
        if (parentId == null) q.isNull("parent_id");
        else q.eq("parent_id", parentId);
        q.orderByAsc("title");
        return list(q);
    }

    public List<KbDocument> search(String keyword, Long spaceId, int limit) {
        QueryWrapper<KbDocument> q = new QueryWrapper<>();
        q.isNull("deleted_at");
        if (spaceId != null) q.eq("space_id", spaceId);
        if (keyword != null && !keyword.isBlank()) {
            q.like("title", keyword);
        }
        q.last("LIMIT " + Math.min(limit, 100));
        q.orderByDesc("updated_at");
        return list(q);
    }

    @Transactional
    public void softDelete(long id) {
        KbDocument d = findActiveOrThrow(id);
        d.setDeletedAt(LocalDateTime.now());
        updateById(d);
        audit.report(currentActor(), "KB_DOCUMENT_DELETED", "kb_document",
                String.valueOf(id), Map.of("title", d.getTitle()));
    }

    private static String currentActor() {
        return CurrentActor.displayName();
    }
}

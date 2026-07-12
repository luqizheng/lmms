package com.coder.pm.infra.service;

import com.baomidou.mybatisplus.core.conditions.query.QueryWrapper;
import com.baomidou.mybatisplus.extension.service.impl.ServiceImpl;
import com.coder.pm.infra.persistence.entity.KbDocumentVersion;
import com.coder.pm.infra.persistence.mapper.KbDocumentVersionMapper;
import com.coder.pm.web.exception.NotFoundException;
import org.springframework.stereotype.Service;
import org.springframework.transaction.annotation.Transactional;

import java.util.List;

@Service
public class KbDocumentVersionService extends ServiceImpl<KbDocumentVersionMapper, KbDocumentVersion> {

    @Transactional
    public KbDocumentVersion appendVersion(long documentId, int versionNo, String title,
                                          String contentMd, String contentHtml,
                                          long editorId, String editSummary) {
        KbDocumentVersion v = new KbDocumentVersion();
        v.setDocumentId(documentId);
        v.setVersionNo(versionNo);
        v.setTitle(title);
        v.setContentMd(contentMd);
        v.setContentHtml(contentHtml);
        v.setEditorId(editorId);
        v.setEditSummary(editSummary);
        save(v);
        return v;
    }

    public int nextVersionNumber(long documentId) {
        QueryWrapper<KbDocumentVersion> q = new QueryWrapper<>();
        q.eq("document_id", documentId).orderByDesc("version_no").last("LIMIT 1");
        KbDocumentVersion latest = getOne(q, false);
        return latest == null ? 1 : latest.getVersionNo() + 1;
    }

    public KbDocumentVersion getByVersionNo(long documentId, int versionNo) {
        QueryWrapper<KbDocumentVersion> q = new QueryWrapper<>();
        q.eq("document_id", documentId).eq("version_no", versionNo).last("LIMIT 1");
        KbDocumentVersion v = getOne(q, false);
        if (v == null) {
            throw new NotFoundException(
                    "version " + versionNo + " not found for document " + documentId);
        }
        return v;
    }

    public KbDocumentVersion getLatest(long documentId) {
        QueryWrapper<KbDocumentVersion> q = new QueryWrapper<>();
        q.eq("document_id", documentId).orderByDesc("version_no").last("LIMIT 1");
        KbDocumentVersion v = getOne(q, false);
        if (v == null) {
            throw new NotFoundException("no versions for document: " + documentId);
        }
        return v;
    }

    public List<KbDocumentVersion> listByDocument(long documentId) {
        QueryWrapper<KbDocumentVersion> q = new QueryWrapper<>();
        q.eq("document_id", documentId).orderByDesc("version_no");
        return list(q);
    }
}

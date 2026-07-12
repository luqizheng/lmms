package com.coder.pm.infra.service;

import com.baomidou.mybatisplus.core.conditions.query.QueryWrapper;
import com.baomidou.mybatisplus.extension.service.impl.ServiceImpl;
import com.coder.pm.infra.persistence.entity.Comment;
import com.coder.pm.infra.persistence.mapper.CommentMapper;
import com.coder.pm.infra.sdk.AuditEventReporter;
import com.coder.pm.infra.security.CurrentActor;
import org.springframework.stereotype.Service;
import org.springframework.transaction.annotation.Transactional;

import java.util.List;

@Service
public class CommentService extends ServiceImpl<CommentMapper, Comment> {

    private final AuditEventReporter audit;

    public CommentService(AuditEventReporter audit) {
        this.audit = audit;
    }

    @Transactional
    public Comment create(Comment draft) {
        if (draft.getAuthorId() == null) {
            Long actorId = CurrentActor.actorIdOrNull();
            if (actorId != null) draft.setAuthorId(actorId);
        }
        save(draft);
        audit.report(CurrentActor.displayName(), "PM_COMMENT_CREATED", "pm_comment",
                String.valueOf(draft.getId()),
                java.util.Map.of("documentId", draft.getDocumentId()));
        return draft;
    }

    public List<Comment> listByDocument(long documentId) {
        QueryWrapper<Comment> q = new QueryWrapper<>();
        q.eq("document_id", documentId).isNull("deleted_at");
        q.orderByAsc("created_at");
        return list(q);
    }
}

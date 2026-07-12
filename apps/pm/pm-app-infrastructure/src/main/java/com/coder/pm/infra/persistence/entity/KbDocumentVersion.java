package com.coder.pm.infra.persistence.entity;

import com.baomidou.mybatisplus.annotation.IdType;
import com.baomidou.mybatisplus.annotation.TableField;
import com.baomidou.mybatisplus.annotation.TableId;
import com.baomidou.mybatisplus.annotation.TableName;

import java.time.LocalDateTime;

/**
 * Immutable snapshot of a {@link KbDocument}'s content at a point in time.
 *
 * <p>Stored as {@code kb_document_versions}. {@code (document_id, version_no)} is unique; the
 * highest version_no for a document is the canonical "current" version, also pointed to by
 * {@link KbDocument#getCurrentVersionId()}.
 *
 * <p>{@code contentMd} is the raw markdown source (LONGTEXT in MySQL); {@code contentHtml} is the
 * rendered HTML cache (LONGTEXT) populated by {@code MarkdownRenderer}.
 */
@TableName("kb_document_versions")
public class KbDocumentVersion {

    @TableId(type = IdType.AUTO)
    private Long id;

    @TableField("document_id")
    private Long documentId;

    @TableField("version_no")
    private Integer versionNo;

    @TableField("title")
    private String title;

    @TableField("content_md")
    private String contentMd;

    @TableField("content_html")
    private String contentHtml;

    @TableField("editor_id")
    private Long editorId;

    @TableField("edit_summary")
    private String editSummary;

    @TableField("created_at")
    private LocalDateTime createdAt;

    public Long getId() { return id; }
    public void setId(Long id) { this.id = id; }

    public Long getDocumentId() { return documentId; }
    public void setDocumentId(Long documentId) { this.documentId = documentId; }

    public Integer getVersionNo() { return versionNo; }
    public void setVersionNo(Integer versionNo) { this.versionNo = versionNo; }

    public String getTitle() { return title; }
    public void setTitle(String title) { this.title = title; }

    public String getContentMd() { return contentMd; }
    public void setContentMd(String contentMd) { this.contentMd = contentMd; }

    public String getContentHtml() { return contentHtml; }
    public void setContentHtml(String contentHtml) { this.contentHtml = contentHtml; }

    public Long getEditorId() { return editorId; }
    public void setEditorId(Long editorId) { this.editorId = editorId; }

    public String getEditSummary() { return editSummary; }
    public void setEditSummary(String editSummary) { this.editSummary = editSummary; }

    public LocalDateTime getCreatedAt() { return createdAt; }
    public void setCreatedAt(LocalDateTime createdAt) { this.createdAt = createdAt; }
}

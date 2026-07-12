package com.kudi.coder.filesdk.model;

import com.fasterxml.jackson.annotation.JsonInclude;

import java.time.Instant;

/**
 * File resource metadata returned by file-service.
 */
@JsonInclude(JsonInclude.Include.NON_NULL)
public class FileInfo {
    private Long id;
    private String name;
    private long size;
    private String contentType;
    private String sha256;
    private String url;
    private Long uploaderId;
    private String tag;
    private Instant createdAt;

    public Long getId() { return id; }
    public void setId(Long id) { this.id = id; }

    public String getName() { return name; }
    public void setName(String name) { this.name = name; }

    public long getSize() { return size; }
    public void setSize(long size) { this.size = size; }

    public String getContentType() { return contentType; }
    public void setContentType(String contentType) { this.contentType = contentType; }

    public String getSha256() { return sha256; }
    public void setSha256(String sha256) { this.sha256 = sha256; }

    public String getUrl() { return url; }
    public void setUrl(String url) { this.url = url; }

    public Long getUploaderId() { return uploaderId; }
    public void setUploaderId(Long uploaderId) { this.uploaderId = uploaderId; }

    public String getTag() { return tag; }
    public void setTag(String tag) { this.tag = tag; }

    public Instant getCreatedAt() { return createdAt; }
    public void setCreatedAt(Instant createdAt) { this.createdAt = createdAt; }
}

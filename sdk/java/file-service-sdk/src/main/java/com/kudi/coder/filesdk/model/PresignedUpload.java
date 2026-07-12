package com.kudi.coder.filesdk.model;

import com.fasterxml.jackson.annotation.JsonInclude;

import java.time.Instant;
import java.util.Map;

/**
 * Result of {@code POST /v1/file/files/presign} — points the client at a presigned URL
 * where the file body can be PUT/POSTed within {@code expiresAt}.
 */
@JsonInclude(JsonInclude.Include.NON_NULL)
public class PresignedUpload {
    private String fileId;
    private String uploadUrl;
    private String method;
    private Instant expiresAt;
    private Map<String, String> headers;

    public String getFileId() { return fileId; }
    public void setFileId(String fileId) { this.fileId = fileId; }

    public String getUploadUrl() { return uploadUrl; }
    public void setUploadUrl(String uploadUrl) { this.uploadUrl = uploadUrl; }

    public String getMethod() { return method; }
    public void setMethod(String method) { this.method = method; }

    public Instant getExpiresAt() { return expiresAt; }
    public void setExpiresAt(Instant expiresAt) { this.expiresAt = expiresAt; }

    public Map<String, String> getHeaders() { return headers; }
    public void setHeaders(Map<String, String> headers) { this.headers = headers; }
}

package com.kudi.coder.filesdk.model;

import com.fasterxml.jackson.annotation.JsonInclude;

/**
 * Metadata supplied to {@code POST /v1/file/files/presign} and the {@code metadata}
 * multipart part of {@code POST /v1/file/files/upload}.
 */
@JsonInclude(JsonInclude.Include.NON_NULL)
public class UploadFileMeta {
    private String name;
    private long size;
    private String contentType;
    private String checksum;
    private String tenantId;
    private String tag;

    public UploadFileMeta() {}

    public static Builder builder() { return new Builder(); }

    public String getName() { return name; }
    public void setName(String name) { this.name = name; }

    public long getSize() { return size; }
    public void setSize(long size) { this.size = size; }

    public String getContentType() { return contentType; }
    public void setContentType(String contentType) { this.contentType = contentType; }

    public String getChecksum() { return checksum; }
    public void setChecksum(String checksum) { this.checksum = checksum; }

    public String getTenantId() { return tenantId; }
    public void setTenantId(String tenantId) { this.tenantId = tenantId; }

    public String getTag() { return tag; }
    public void setTag(String tag) { this.tag = tag; }

    public static final class Builder {
        private final UploadFileMeta m = new UploadFileMeta();

        public Builder name(String v) { m.name = v; return this; }
        public Builder size(long v) { m.size = v; return this; }
        public Builder contentType(String v) { m.contentType = v; return this; }
        public Builder checksum(String v) { m.checksum = v; return this; }
        public Builder tenantId(String v) { m.tenantId = v; return this; }
        public Builder tag(String v) { m.tag = v; return this; }

        public UploadFileMeta build() { return m; }
    }
}

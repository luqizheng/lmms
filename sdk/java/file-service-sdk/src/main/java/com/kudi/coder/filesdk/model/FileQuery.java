package com.kudi.coder.filesdk.model;

/**
 * Query parameters for {@code GET /v1/file/files}. Use {@code tenantId} or {@code tag} to
 * narrow the listing; pagination via {@code page} / {@code size}.
 */
public class FileQuery {
    private String tenantId;
    private String tag;
    private int page = 1;
    private int size = 20;

    public String getTenantId() { return tenantId; }
    public void setTenantId(String tenantId) { this.tenantId = tenantId; }

    public String getTag() { return tag; }
    public void setTag(String tag) { this.tag = tag; }

    public int getPage() { return page; }
    public void setPage(int page) { this.page = page; }

    public int getSize() { return size; }
    public void setSize(int size) { this.size = size; }
}

package com.kudi.coder.configsdk.model;

/**
 * Query for {@code GET /v1/config/namespaces}. Empty fields are omitted from the URL.
 */
public class NamespaceQuery {
    private String app;
    private int page = 1;
    private int size = 20;

    public String getApp() { return app; }
    public void setApp(String app) { this.app = app; }

    public int getPage() { return page; }
    public void setPage(int page) { this.page = page; }

    public int getSize() { return size; }
    public void setSize(int size) { this.size = size; }
}

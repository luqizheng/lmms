package com.kudi.coder.configsdk.model;

public class ConfigItemQuery {
    private String key;
    private int page = 1;
    private int size = 20;

    public String getKey() { return key; }
    public void setKey(String key) { this.key = key; }

    public int getPage() { return page; }
    public void setPage(int page) { this.page = page; }

    public int getSize() { return size; }
    public void setSize(int size) { this.size = size; }
}

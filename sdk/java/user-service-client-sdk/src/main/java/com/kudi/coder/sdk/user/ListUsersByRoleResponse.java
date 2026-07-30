package com.kudi.coder.sdk.user;

import com.fasterxml.jackson.annotation.JsonIgnoreProperties;

import java.util.List;

@JsonIgnoreProperties(ignoreUnknown = true)
public class ListUsersByRoleResponse {
    private List<User> items;
    private long total;
    private int page;
    private int size;

    public List<User> getItems() { return items; }
    public void setItems(List<User> items) { this.items = items; }
    public long getTotal() { return total; }
    public void setTotal(long total) { this.total = total; }
    public int getPage() { return page; }
    public void setPage(int page) { this.page = page; }
    public int getSize() { return size; }
    public void setSize(int size) { this.size = size; }
}

package com.kudi.coder.auditsdk.model;

import com.fasterxml.jackson.annotation.JsonInclude;
import java.util.Collections;
import java.util.List;

@JsonInclude(JsonInclude.Include.NON_NULL)
public class PageResult<T> {
    private List<T> items;
    private long total;
    private int page;
    private int size;

    public PageResult() { this.items = Collections.emptyList(); }

    public PageResult(List<T> items, long total, int page, int size) {
        this.items = items == null ? Collections.emptyList() : items;
        this.total = total;
        this.page = page;
        this.size = size;
    }

    public List<T> getItems() { return items; }
    public void setItems(List<T> items) { this.items = items == null ? Collections.emptyList() : items; }
    public long getTotal() { return total; }
    public void setTotal(long total) { this.total = total; }
    public int getPage() { return page; }
    public void setPage(int page) { this.page = page; }
    public int getSize() { return size; }
    public void setSize(int size) { this.size = size; }
}
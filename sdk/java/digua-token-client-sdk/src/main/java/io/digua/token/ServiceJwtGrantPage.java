package io.digua.token;

import com.fasterxml.jackson.annotation.JsonIgnoreProperties;
import com.fasterxml.jackson.annotation.JsonProperty;

import java.util.ArrayList;
import java.util.List;

/**
 * grant 分页查询结果：{@code {items, total, page, size}}。
 */
@JsonIgnoreProperties(ignoreUnknown = true)
public class ServiceJwtGrantPage {
    @JsonProperty("items")
    private List<ServiceJwtGrantInfo> items = new ArrayList<>();

    @JsonProperty("total")
    private long total;

    @JsonProperty("page")
    private int page;

    @JsonProperty("size")
    private int size;

    public List<ServiceJwtGrantInfo> getItems() { return items; }
    public void setItems(List<ServiceJwtGrantInfo> items) { this.items = items; }

    public long getTotal() { return total; }
    public void setTotal(long total) { this.total = total; }

    public int getPage() { return page; }
    public void setPage(int page) { this.page = page; }

    public int getSize() { return size; }
    public void setSize(int size) { this.size = size; }
}

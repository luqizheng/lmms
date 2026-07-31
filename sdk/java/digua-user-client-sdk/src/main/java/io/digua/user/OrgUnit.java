package io.digua.user;

import com.fasterxml.jackson.annotation.JsonIgnoreProperties;
import com.fasterxml.jackson.annotation.JsonProperty;

import java.util.List;

@JsonIgnoreProperties(ignoreUnknown = true)
public class OrgUnit {
    private String id;
    @JsonProperty("parent_id")
    private String parentId;
    private String name;
    private String code;
    private int level;
    @JsonProperty("sort_order")
    private int sortOrder;
    private String status;
    private String path;
    private List<OrgUnit> children;

    public String getId() { return id; }
    public void setId(String id) { this.id = id; }
    public String getParentId() { return parentId; }
    public void setParentId(String parentId) { this.parentId = parentId; }
    public String getName() { return name; }
    public void setName(String name) { this.name = name; }
    public String getCode() { return code; }
    public void setCode(String code) { this.code = code; }
    public int getLevel() { return level; }
    public void setLevel(int level) { this.level = level; }
    public int getSortOrder() { return sortOrder; }
    public void setSortOrder(int sortOrder) { this.sortOrder = sortOrder; }
    public String getStatus() { return status; }
    public void setStatus(String status) { this.status = status; }
    public String getPath() { return path; }
    public void setPath(String path) { this.path = path; }
    public List<OrgUnit> getChildren() { return children; }
    public void setChildren(List<OrgUnit> children) { this.children = children; }
}

package io.digua.user;

import com.fasterxml.jackson.annotation.JsonProperty;

public class QueryUsersByOrgAndRoleRequest {
    @JsonProperty("org_path")
    private String orgPath;
    @JsonProperty("role_name")
    private String roleName;

    public QueryUsersByOrgAndRoleRequest() {}

    public QueryUsersByOrgAndRoleRequest(String orgPath, String roleName) {
        this.orgPath = orgPath;
        this.roleName = roleName;
    }

    public String getOrgPath() { return orgPath; }
    public void setOrgPath(String orgPath) { this.orgPath = orgPath; }
    public String getRoleName() { return roleName; }
    public void setRoleName(String roleName) { this.roleName = roleName; }
}

package io.digua.token;

import com.fasterxml.jackson.annotation.JsonIgnoreProperties;
import com.fasterxml.jackson.annotation.JsonProperty;

/**
 * Service JWT audience grant（owner → audience 签发授权关系）。
 *
 * <p>WP-LMMS-SERVICE-JWT-AUDIENCE-ADMIN-01。不含任何凭据材料；
 * {@code systemManaged=true} 表示内置系统授权，服务端拒绝改删（HTTP 409）。
 */
@JsonIgnoreProperties(ignoreUnknown = true)
public class ServiceJwtGrantInfo {
    @JsonProperty("owner")
    private String owner;

    @JsonProperty("audience")
    private String audience;

    /** {@code active} | {@code disabled} */
    @JsonProperty("status")
    private String status;

    @JsonProperty("system_managed")
    private boolean systemManaged;

    @JsonProperty("created_at")
    private String createdAt;

    @JsonProperty("updated_at")
    private String updatedAt;

    public String getOwner() { return owner; }
    public void setOwner(String owner) { this.owner = owner; }

    public String getAudience() { return audience; }
    public void setAudience(String audience) { this.audience = audience; }

    public String getStatus() { return status; }
    public void setStatus(String status) { this.status = status; }

    public boolean isSystemManaged() { return systemManaged; }
    public void setSystemManaged(boolean systemManaged) { this.systemManaged = systemManaged; }

    public String getCreatedAt() { return createdAt; }
    public void setCreatedAt(String createdAt) { this.createdAt = createdAt; }

    public String getUpdatedAt() { return updatedAt; }
    public void setUpdatedAt(String updatedAt) { this.updatedAt = updatedAt; }
}

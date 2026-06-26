package com.kudi.coder.sdk.common.model;

import com.fasterxml.jackson.annotation.JsonIgnoreProperties;
import com.fasterxml.jackson.annotation.JsonProperty;

@JsonIgnoreProperties(ignoreUnknown = true)
public class ResourceTokenInfo {
    @JsonProperty("resource_token")
    private String resourceToken;

    @JsonProperty("expires_in")
    private long expiresIn;

    public ResourceTokenInfo() {}

    public String getResourceToken() { return resourceToken; }
    public void setResourceToken(String resourceToken) { this.resourceToken = resourceToken; }

    public long getExpiresIn() { return expiresIn; }
    public void setExpiresIn(long expiresIn) { this.expiresIn = expiresIn; }
}

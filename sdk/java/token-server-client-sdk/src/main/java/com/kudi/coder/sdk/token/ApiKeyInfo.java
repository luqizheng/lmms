package com.kudi.coder.sdk.token;

import com.fasterxml.jackson.annotation.JsonIgnoreProperties;
import com.fasterxml.jackson.annotation.JsonProperty;

@JsonIgnoreProperties(ignoreUnknown = true)
public class ApiKeyInfo {
    @JsonProperty("api_key_id")
    private String apiKeyId;

    @JsonProperty("api_key_secret")
    private String apiKeySecret;

    public String getApiKeyId() { return apiKeyId; }
    public void setApiKeyId(String apiKeyId) { this.apiKeyId = apiKeyId; }
    public String getApiKeySecret() { return apiKeySecret; }
    public void setApiKeySecret(String apiKeySecret) { this.apiKeySecret = apiKeySecret; }
}

package com.kudi.coder.sdk.common.model;

import com.fasterxml.jackson.annotation.JsonIgnoreProperties;
import com.fasterxml.jackson.annotation.JsonProperty;

@JsonIgnoreProperties(ignoreUnknown = true)
public class ApiError {
    private String error;
    private String message;

    @JsonProperty("error")
    public String getError() { return error; }
    public void setError(String error) { this.error = error; }

    @JsonProperty("message")
    public String getMessage() { return message; }
    public void setMessage(String message) { this.message = message; }
}

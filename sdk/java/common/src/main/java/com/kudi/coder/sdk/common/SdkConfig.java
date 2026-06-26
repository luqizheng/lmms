package com.kudi.coder.sdk.common;

public class SdkConfig {
    private String baseUrl;
    private int connectTimeoutMs = 5000;
    private int readTimeoutMs = 15000;
    private int retryCount = 1;
    private String userAgent = "kudi-coder-sdk/0.2.0";

    public SdkConfig() {}

    public SdkConfig(String baseUrl) {
        this.baseUrl = baseUrl;
    }

    public String getBaseUrl() { return baseUrl; }
    public void setBaseUrl(String baseUrl) { this.baseUrl = baseUrl; }

    public int getConnectTimeoutMs() { return connectTimeoutMs; }
    public void setConnectTimeoutMs(int v) { this.connectTimeoutMs = v; }

    public int getReadTimeoutMs() { return readTimeoutMs; }
    public void setReadTimeoutMs(int v) { this.readTimeoutMs = v; }

    public int getRetryCount() { return retryCount; }
    public void setRetryCount(int v) { this.retryCount = v; }

    public String getUserAgent() { return userAgent; }
    public void setUserAgent(String userAgent) { this.userAgent = userAgent; }
}

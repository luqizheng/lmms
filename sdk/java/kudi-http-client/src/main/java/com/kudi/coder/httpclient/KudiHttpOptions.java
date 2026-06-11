package com.kudi.coder.httpclient;

import java.time.Duration;

public class KudiHttpOptions {
    public String tokenEndpoint = "http://token-svc/api/tokens/access";
    public Duration connectTimeout = Duration.ofSeconds(5);
    public Duration readTimeout = Duration.ofSeconds(15);
    public int retryCount = 1;
    public Duration refreshSkew = Duration.ofSeconds(60);
    public String userAgent = "kudi-http-client/0.1.0";
}

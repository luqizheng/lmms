package com.kudi.coder.httpclient;

import java.time.Duration;

public class KudiHttpOptions {
    public Duration connectTimeout = Duration.ofSeconds(5);
    public Duration readTimeout = Duration.ofSeconds(15);
    public int retryCount = 1;
    public String userAgent = "kudi-http-client/0.2.0";
}

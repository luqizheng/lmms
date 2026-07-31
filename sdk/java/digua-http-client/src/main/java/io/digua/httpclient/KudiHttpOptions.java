package io.digua.httpclient;

import java.time.Duration;

public class KudiHttpOptions {
    public Duration connectTimeout = Duration.ofSeconds(5);
    public Duration readTimeout = Duration.ofSeconds(15);
    public int retryCount = 1;
    public String userAgent = "digua-http-client/1.0.0";
}

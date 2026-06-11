package com.kudi.coder.httpclient.Exceptions;

public class RateLimitException extends KudiHttpException {
    public RateLimitException(String message) {
        super(message, 429, null);
    }

    public RateLimitException(String message, String details) {
        super(message, 429, details);
    }
}

package com.kudi.coder.sdk.common.exception;

public class RateLimitException extends SdkException {
    public RateLimitException(String message) {
        super(message, 429, null, null);
    }

    public RateLimitException(String message, String details) {
        super(message, 429, details, null);
    }
}

package com.kudi.coder.sdk.common.exception;

public class SdkException extends RuntimeException {
    private final int statusCode;
    private final String details;

    public SdkException(String message) {
        this(message, 0, null, null);
    }

    public SdkException(String message, Throwable cause) {
        this(message, 0, null, cause);
    }

    public SdkException(String message, int statusCode, String details, Throwable cause) {
        super(message, cause);
        this.statusCode = statusCode;
        this.details = details;
    }

    public int getStatusCode() {
        return statusCode;
    }

    public String getDetails() {
        return details;
    }
}

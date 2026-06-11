package com.kudi.coder.httpclient.Exceptions;

public class KudiHttpException extends RuntimeException {
    private final int statusCode;
    private final String details;

    public KudiHttpException(String message) {
        this(message, 0, null);
    }

    public KudiHttpException(String message, int statusCode, String details) {
        super(message);
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

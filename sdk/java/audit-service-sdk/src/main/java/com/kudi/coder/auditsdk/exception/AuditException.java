package com.kudi.coder.auditsdk.exception;

public class AuditException extends RuntimeException {
    private final int statusCode;

    public AuditException(String message, int statusCode) {
        super(message);
        this.statusCode = statusCode;
    }

    public AuditException(String message, int statusCode, Throwable cause) {
        super(message, cause);
        this.statusCode = statusCode;
    }

    public int getStatusCode() { return statusCode; }
}
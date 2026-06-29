package com.kudi.coder.auditsdk.exception;

public class AuditRateLimitException extends AuditException {
    public AuditRateLimitException(String message) {
        super(message, 429);
    }
}
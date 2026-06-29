package com.kudi.coder.auditsdk.exception;

public class AuditAuthException extends AuditException {
    public AuditAuthException(String message) {
        super(message, 401);
    }
}
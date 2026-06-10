package com.kudi.coder.sdk.common.exception;

public class ValidationException extends SdkException {
    public ValidationException(String message) {
        super(message, 400, null, null);
    }

    public ValidationException(String message, int statusCode, String details) {
        super(message, statusCode, details, null);
    }
}

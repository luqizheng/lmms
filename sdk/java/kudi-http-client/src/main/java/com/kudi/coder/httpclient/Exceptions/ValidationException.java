package com.kudi.coder.httpclient.Exceptions;

public class ValidationException extends KudiHttpException {
    public ValidationException(String message) {
        super(message, 400, null);
    }

    public ValidationException(String message, int statusCode, String details) {
        super(message, statusCode, details);
    }
}

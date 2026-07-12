package com.kudi.coder.filesdk.exception;

/**
 * Base exception for {@code file-service-sdk}.
 */
public class FileException extends RuntimeException {
    private final int statusCode;

    public FileException(String message, int statusCode) {
        super(message);
        this.statusCode = statusCode;
    }

    public FileException(String message, int statusCode, Throwable cause) {
        super(message, cause);
        this.statusCode = statusCode;
    }

    public int getStatusCode() {
        return statusCode;
    }
}

package com.kudi.coder.filesdk.exception;

public class FileRateLimitException extends FileException {
    public FileRateLimitException(String message) {
        super(message, 429);
    }
}

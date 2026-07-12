package com.kudi.coder.filesdk.exception;

public class FileAuthException extends FileException {
    public FileAuthException(String message) {
        super(message, 401);
    }
}

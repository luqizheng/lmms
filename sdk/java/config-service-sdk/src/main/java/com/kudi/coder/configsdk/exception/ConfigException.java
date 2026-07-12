package com.kudi.coder.configsdk.exception;

public class ConfigException extends RuntimeException {
    private final int statusCode;

    public ConfigException(String message, int statusCode) {
        super(message);
        this.statusCode = statusCode;
    }

    public ConfigException(String message, int statusCode, Throwable cause) {
        super(message, cause);
        this.statusCode = statusCode;
    }

    public int getStatusCode() {
        return statusCode;
    }
}

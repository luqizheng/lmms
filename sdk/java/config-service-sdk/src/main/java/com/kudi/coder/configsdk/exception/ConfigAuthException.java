package com.kudi.coder.configsdk.exception;

public class ConfigAuthException extends ConfigException {
    public ConfigAuthException(String message) {
        super(message, 401);
    }
}

package com.kudi.coder.configsdk.exception;

public class ConfigRateLimitException extends ConfigException {
    public ConfigRateLimitException(String message) {
        super(message, 429);
    }
}

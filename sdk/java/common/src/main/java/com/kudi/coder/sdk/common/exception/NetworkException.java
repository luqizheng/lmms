package com.kudi.coder.sdk.common.exception;

public class NetworkException extends SdkException {
    public NetworkException(String message) {
        super(message, 0, null, null);
    }

    public NetworkException(String message, Throwable cause) {
        super(message, 0, null, cause);
    }
}

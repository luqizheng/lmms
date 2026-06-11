package com.kudi.coder.httpclient.Exceptions;

public class NetworkException extends KudiHttpException {
    public NetworkException(String message) {
        super(message);
    }

    public NetworkException(String message, Throwable cause) {
        super(message);
        initCause(cause);
    }
}

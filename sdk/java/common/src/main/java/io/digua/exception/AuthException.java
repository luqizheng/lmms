package io.digua.exception;

public class AuthException extends SdkException {
    public AuthException(String message) {
        super(message, 401, null, null);
    }

    public AuthException(String message, int statusCode, String details) {
        super(message, statusCode, details, null);
    }
}

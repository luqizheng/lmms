package io.digua.Exceptions;

public class AuthException extends KudiHttpException {
    public AuthException(String message) {
        super(message, 401, null);
    }

    public AuthException(String message, int statusCode, String details) {
        super(message, statusCode, details);
    }
}

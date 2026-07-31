package io.digua.Exceptions;

public class ServerException extends KudiHttpException {
    public ServerException(String message) {
        super(message, 500, null);
    }

    public ServerException(String message, int statusCode, String details) {
        super(message, statusCode, details);
    }
}

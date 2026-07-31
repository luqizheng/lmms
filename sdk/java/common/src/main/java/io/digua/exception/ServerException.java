package io.digua.exception;

public class ServerException extends SdkException {
    public ServerException(String message) {
        super(message, 500, null, null);
    }

    public ServerException(String message, int statusCode, String details) {
        super(message, statusCode, details, null);
    }
}

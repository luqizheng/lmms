package io.digua.exception;

/**
 * Base exception for {@code notification-service-sdk}. Maps to non-2xx responses other than
 * 401 / 429.
 */
public class NotificationException extends RuntimeException {
    private final int statusCode;

    public NotificationException(String message, int statusCode) {
        super(message);
        this.statusCode = statusCode;
    }

    public NotificationException(String message, int statusCode, Throwable cause) {
        super(message, cause);
        this.statusCode = statusCode;
    }

    public int getStatusCode() {
        return statusCode;
    }
}

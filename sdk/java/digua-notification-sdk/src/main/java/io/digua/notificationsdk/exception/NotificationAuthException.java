package io.digua.exception;

/**
 * Thrown when notification-service returns 401. Indicates an invalid/missing JWT or
 * insufficient permissions.
 */
public class NotificationAuthException extends NotificationException {
    public NotificationAuthException(String message) {
        super(message, 401);
    }
}

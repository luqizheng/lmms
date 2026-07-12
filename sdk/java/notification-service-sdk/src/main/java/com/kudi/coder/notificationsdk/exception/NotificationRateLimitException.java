package com.kudi.coder.notificationsdk.exception;

/**
 * Thrown when notification-service returns 429. Caller should back off and retry.
 */
public class NotificationRateLimitException extends NotificationException {
    public NotificationRateLimitException(String message) {
        super(message, 429);
    }
}

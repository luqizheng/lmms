package io.digua.web;

import io.digua.notificationsdk.NotificationServiceClient;
import io.digua.shared.ApiResponse;
import org.springframework.web.bind.annotation.GetMapping;
import org.springframework.web.bind.annotation.RequestMapping;
import org.springframework.web.bind.annotation.RestController;

@RestController
@RequestMapping("/demo/notify")
public class NotificationDemoController {

    private final NotificationServiceClient notificationClient;

    public NotificationDemoController(NotificationServiceClient notificationClient) {
        this.notificationClient = notificationClient;
    }

    @GetMapping("/unread-count")
    public ApiResponse<Long> unreadCount() {
        return ApiResponse.ok(notificationClient.unreadCount());
    }
}
package io.digua.notificationsdk;

import io.digua.exception.NotificationAuthException;
import io.digua.exception.NotificationException;
import io.digua.exception.NotificationRateLimitException;
import io.digua.model.Announcement;
import io.digua.model.AnnouncementRequest;
import io.digua.model.PageResult;
import okhttp3.mockwebserver.MockResponse;
import okhttp3.mockwebserver.MockWebServer;
import okhttp3.mockwebserver.RecordedRequest;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;

import java.util.Arrays;
import java.util.concurrent.atomic.AtomicReference;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

class NotificationServiceClientTest {

    private MockWebServer server;
    private NotificationServiceClient client;
    private AtomicReference<String> bearer;

    @BeforeEach
    void setUp() throws Exception {
        server = new MockWebServer();
        server.start();
        bearer = new AtomicReference<>("test-token");
        client = new NotificationServiceClient(server.url("/").toString(), bearer::get);
    }

    @AfterEach
    void tearDown() throws Exception {
        server.shutdown();
    }

    @Test
    void createAnnouncement_returns_announcement_on_200() throws Exception {
        server.enqueue(new MockResponse()
                .setResponseCode(200)
                .setBody("{\"id\":1,\"title\":\"pm-release\",\"status\":\"draft\",\"level\":\"info\",\"pinned\":false}")
                .addHeader("Content-Type", "application/json"));

        Announcement created = client.createAnnouncement(
                AnnouncementRequest.builder()
                        .title("pm-release")
                        .content("PM 0.1.0")
                        .audience("all")
                        .level("info")
                        .pinned(false)
                        .channels(Arrays.asList("web", "ws"))
                        .build());

        assertEquals(1L, created.getId());
        assertEquals("pm-release", created.getTitle());
        assertEquals("draft", created.getStatus());

        RecordedRequest req = server.takeRequest();
        assertEquals("POST", req.getMethod());
        assertEquals("/v1/notification/announcements", req.getPath());
        assertEquals("Bearer test-token", req.getHeader("Authorization"));
        assertTrue(req.getBody().readUtf8().contains("\"title\":\"pm-release\""));
    }

    @Test
    void listMyAnnouncements_parses_pagination_with_pinned_filter() throws Exception {
        server.enqueue(new MockResponse()
                .setResponseCode(200)
                .setBody("{\"items\":[{\"id\":1,\"title\":\"a\"}],\"total\":1,\"page\":1,\"size\":20}")
                .addHeader("Content-Type", "application/json"));

        PageResult<Announcement> page = client.listMyAnnouncements(true, 1, 20);
        assertEquals(1L, page.getTotal());
        assertEquals("a", page.getItems().get(0).getTitle());

        RecordedRequest req = server.takeRequest();
        assertEquals("/v1/notification/announcements?pinned=true&page=1&size=20", req.getPath());
    }

    @Test
    void publishAnnouncement_sends_post_with_id() throws Exception {
        server.enqueue(new MockResponse()
                .setResponseCode(200)
                .setBody("{\"id\":42,\"status\":\"published\"}")
                .addHeader("Content-Type", "application/json"));

        Announcement result = client.publishAnnouncement(42L);
        assertEquals(42L, result.getId());
        assertEquals("published", result.getStatus());

        RecordedRequest req = server.takeRequest();
        assertEquals("POST", req.getMethod());
        assertEquals("/v1/notification/announcements/42/publish", req.getPath());
    }

    @Test
    void publishAnnouncement_throws_AuthException_on_401() {
        server.enqueue(new MockResponse().setResponseCode(401).setBody("unauthorized"));
        assertThrows(NotificationAuthException.class, () -> client.publishAnnouncement(1L));
    }

    @Test
    void unreadCount_returns_long() throws Exception {
        server.enqueue(new MockResponse()
                .setResponseCode(200)
                .setBody("[7]")
                .addHeader("Content-Type", "application/json"));
        long count = client.unreadCount();
        assertEquals(7L, count);
    }

    @Test
    void markAsRead_sends_post_id_read() throws Exception {
        server.enqueue(new MockResponse().setResponseCode(200).setBody(""));
        client.markAsRead(99L);
        RecordedRequest req = server.takeRequest();
        assertEquals("POST", req.getMethod());
        assertEquals("/v1/notification/announcements/99/read", req.getPath());
    }

    @Test
    void createAnnouncement_throws_RateLimitException_on_429() {
        server.enqueue(new MockResponse().setResponseCode(429).setBody("rate limit"));
        AnnouncementRequest req = AnnouncementRequest.builder().title("x").content("y").build();
        assertThrows(NotificationRateLimitException.class, () -> client.createAnnouncement(req));
    }

    @Test
    void createAnnouncement_throws_NotificationException_on_500() {
        server.enqueue(new MockResponse().setResponseCode(500).setBody("server err"));
        AnnouncementRequest req = AnnouncementRequest.builder().title("x").content("y").build();
        assertThrows(NotificationException.class, () -> client.createAnnouncement(req));
    }
}

package io.digua.notificationsdk;

import com.fasterxml.jackson.core.type.TypeReference;
import com.fasterxml.jackson.databind.DeserializationFeature;
import com.fasterxml.jackson.databind.ObjectMapper;
import com.fasterxml.jackson.datatype.jsr310.JavaTimeModule;
import io.digua.exception.NotificationAuthException;
import io.digua.exception.NotificationException;
import io.digua.exception.NotificationRateLimitException;
import io.digua.model.Announcement;
import io.digua.model.AnnouncementRequest;
import io.digua.model.PageResult;
import okhttp3.MediaType;
import okhttp3.OkHttpClient;
import okhttp3.Request;
import okhttp3.RequestBody;
import okhttp3.Response;
import okhttp3.ResponseBody;

import java.io.IOException;
import java.util.List;
import java.util.concurrent.TimeUnit;
import java.util.function.Supplier;

/**
 * Java 8 facade for notification-service.
 *
 * <p>Construct with the api-gateway base URL (e.g. {@code http://localhost:3000}) and a
 * {@link Supplier} that yields the current {@code Bearer} token on each request — typically
 * backed by a {@code TokenCache}.
 *
 * <p>Covers MVP endpoints used by apps/pm (project management). See
 * {@code docs/superpowers/specs/2026-07-11-pm-platform-design.md} §4.2 for scope.
 */
public class NotificationServiceClient {

    private static final MediaType JSON = MediaType.parse("application/json; charset=utf-8");
    private static final TypeReference<PageResult<Announcement>> PAGE_TYPE =
            new TypeReference<PageResult<Announcement>>() {};

    private final String baseUrl;
    private final Supplier<String> bearerSupplier;
    private final OkHttpClient http;
    private final ObjectMapper mapper;

    public NotificationServiceClient(String baseUrl, Supplier<String> bearerSupplier) {
        this(baseUrl, bearerSupplier, 5000);
    }

    public NotificationServiceClient(String baseUrl, Supplier<String> bearerSupplier, int timeoutMs) {
        this.baseUrl = baseUrl.endsWith("/") ? baseUrl.substring(0, baseUrl.length() - 1) : baseUrl;
        this.bearerSupplier = bearerSupplier;
        this.http = new OkHttpClient.Builder()
                .connectTimeout(timeoutMs, TimeUnit.MILLISECONDS)
                .readTimeout(timeoutMs, TimeUnit.MILLISECONDS)
                .writeTimeout(timeoutMs, TimeUnit.MILLISECONDS)
                .build();
        this.mapper = new ObjectMapper()
                .registerModule(new JavaTimeModule())
                .configure(DeserializationFeature.FAIL_ON_UNKNOWN_PROPERTIES, false);
    }

    public Announcement createAnnouncement(AnnouncementRequest req) {
        try {
            String body = mapper.writeValueAsString(req);
            Request httpReq = baseRequest("/v1/notification/announcements")
                    .post(RequestBody.create(body, JSON))
                    .build();
            try (Response resp = http.newCall(httpReq).execute()) {
                String respBody = readBody(resp);
                ensureSuccess(resp, respBody);
                return mapper.readValue(respBody, Announcement.class);
            }
        } catch (IOException e) {
            throw new NotificationException("notification createAnnouncement IO error: " + e.getMessage(), 0, e);
        }
    }

    public PageResult<Announcement> listMyAnnouncements(boolean pinned, int page, int size) {
        StringBuilder url = new StringBuilder("/v1/notification/announcements?");
        if (pinned) url.append("pinned=true&");
        url.append("page=").append(page).append("&size=").append(size);
        try {
            Request httpReq = baseRequest(url.toString()).get().build();
            try (Response resp = http.newCall(httpReq).execute()) {
                String respBody = readBody(resp);
                ensureSuccess(resp, respBody);
                return mapper.readValue(respBody, PAGE_TYPE);
            }
        } catch (IOException e) {
            throw new NotificationException("notification listMyAnnouncements IO error: " + e.getMessage(), 0, e);
        }
    }

    public Announcement publishAnnouncement(long id) {
        try {
            Request httpReq = baseRequest("/v1/notification/announcements/" + id + "/publish")
                    .post(RequestBody.create("", JSON))
                    .build();
            try (Response resp = http.newCall(httpReq).execute()) {
                String respBody = readBody(resp);
                ensureSuccess(resp, respBody);
                return mapper.readValue(respBody, Announcement.class);
            }
        } catch (IOException e) {
            throw new NotificationException("notification publishAnnouncement IO error: " + e.getMessage(), 0, e);
        }
    }

    public long unreadCount() {
        try {
            Request httpReq = baseRequest("/v1/notification/unread-count").get().build();
            try (Response resp = http.newCall(httpReq).execute()) {
                String respBody = readBody(resp);
                ensureSuccess(resp, respBody);
                @SuppressWarnings("unchecked")
                List<Object> arr = mapper.readValue(respBody, List.class);
                return arr.isEmpty() ? 0L : Long.parseLong(arr.get(0).toString());
            }
        } catch (IOException e) {
            throw new NotificationException("notification unreadCount IO error: " + e.getMessage(), 0, e);
        }
    }

    public void markAsRead(long id) {
        try {
            Request httpReq = baseRequest("/v1/notification/announcements/" + id + "/read")
                    .post(RequestBody.create("", JSON))
                    .build();
            try (Response resp = http.newCall(httpReq).execute()) {
                String respBody = readBody(resp);
                ensureSuccess(resp, respBody);
            }
        } catch (IOException e) {
            throw new NotificationException("notification markAsRead IO error: " + e.getMessage(), 0, e);
        }
    }

    private Request.Builder baseRequest(String path) {
        Request.Builder b = new Request.Builder().url(baseUrl + path);
        String bearer = bearerSupplier == null ? null : bearerSupplier.get();
        if (bearer != null && !bearer.isEmpty()) {
            b.header("Authorization", "Bearer " + bearer);
        }
        b.header("Content-Type", "application/json");
        b.header("Accept", "application/json");
        return b;
    }

    private void ensureSuccess(Response resp, String body) {
        int code = resp.code();
        if (code >= 200 && code < 300) return;
        if (code == 401) throw new NotificationAuthException("notification 401: " + body);
        if (code == 429) throw new NotificationRateLimitException("notification 429: " + body);
        throw new NotificationException("notification " + code + ": " + body, code);
    }

    private String readBody(Response resp) throws IOException {
        ResponseBody b = resp.body();
        return b == null ? "" : b.string();
    }

    private static String encode(String s) {
        try {
            return java.net.URLEncoder.encode(s, java.nio.charset.StandardCharsets.UTF_8.name());
        } catch (java.io.UnsupportedEncodingException e) {
            throw new IllegalStateException("UTF-8 is a JVM-mandatory charset", e);
        }
    }
}

package com.kudi.coder.auditsdk;

import com.fasterxml.jackson.core.type.TypeReference;
import com.fasterxml.jackson.databind.DeserializationFeature;
import com.fasterxml.jackson.databind.ObjectMapper;
import com.fasterxml.jackson.datatype.jsr310.JavaTimeModule;
import com.kudi.coder.auditsdk.exception.AuditAuthException;
import com.kudi.coder.auditsdk.exception.AuditException;
import com.kudi.coder.auditsdk.exception.AuditRateLimitException;
import com.kudi.coder.auditsdk.model.AuditEvent;
import com.kudi.coder.auditsdk.model.AuditLog;
import com.kudi.coder.auditsdk.model.AuditLogQuery;
import com.kudi.coder.auditsdk.model.PageResult;
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

public class AuditServiceClient {

    private static final MediaType JSON = MediaType.parse("application/json; charset=utf-8");
    private static final TypeReference<PageResult<AuditLog>> PAGE_TYPE = new TypeReference<PageResult<AuditLog>>() {};

    private final String baseUrl;
    private final Supplier<String> bearerSupplier;
    private final OkHttpClient http;
    private final ObjectMapper mapper;

    public AuditServiceClient(String baseUrl, Supplier<String> bearerSupplier) {
        this(baseUrl, bearerSupplier, 5000);
    }

    public AuditServiceClient(String baseUrl, Supplier<String> bearerSupplier, int timeoutMs) {
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

    public AuditLog sendEvent(AuditEvent event) {
        try {
            String body = mapper.writeValueAsString(event);
            Request req = baseRequest("/v1/audit/events")
                    .post(RequestBody.create(body, JSON))
                    .build();
            try (Response resp = http.newCall(req).execute()) {
                String respBody = readBody(resp);
                ensureSuccess(resp, respBody);
                return mapper.readValue(respBody, AuditLog.class);
            }
        } catch (IOException e) {
            throw new AuditException("audit sendEvent IO error: " + e.getMessage(), 0, e);
        }
    }

    public PageResult<AuditLog> queryLogs(AuditLogQuery q) {
        StringBuilder url = new StringBuilder("/v1/audit/logs?");
        if (q.getActor() != null) url.append("actor=").append(encode(q.getActor())).append('&');
        if (q.getAction() != null) url.append("action=").append(encode(q.getAction())).append('&');
        if (q.getResourceType() != null) url.append("resource_type=").append(encode(q.getResourceType())).append('&');
        if (q.getResourceId() != null) url.append("resource_id=").append(encode(q.getResourceId())).append('&');
        url.append("page=").append(q.getPage()).append("&size=").append(q.getSize());
        try {
            Request req = baseRequest(url.toString()).get().build();
            try (Response resp = http.newCall(req).execute()) {
                String respBody = readBody(resp);
                ensureSuccess(resp, respBody);
                return mapper.readValue(respBody, PAGE_TYPE);
            }
        } catch (IOException e) {
            throw new AuditException("audit queryLogs IO error: " + e.getMessage(), 0, e);
        }
    }

    @SuppressWarnings("unchecked")
    public List<String> listEventTypes() {
        try {
            Request req = baseRequest("/v1/audit/event-types").get().build();
            try (Response resp = http.newCall(req).execute()) {
                String respBody = readBody(resp);
                ensureSuccess(resp, respBody);
                return mapper.readValue(respBody, List.class);
            }
        } catch (IOException e) {
            throw new AuditException("audit listEventTypes IO error: " + e.getMessage(), 0, e);
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
        if (code == 401) throw new AuditAuthException("audit 401: " + body);
        if (code == 429) throw new AuditRateLimitException("audit 429: " + body);
        throw new AuditException("audit " + code + ": " + body, code);
    }

    private String readBody(Response resp) throws IOException {
        ResponseBody b = resp.body();
        return b == null ? "" : b.string();
    }

    private static String encode(String s) {
        try {
            return java.net.URLEncoder.encode(s, java.nio.charset.StandardCharsets.UTF_8.name());
        } catch (java.io.UnsupportedEncodingException e) {
            // ponytail: UTF-8 是 JVM 必有字符集，此分支不可达
            throw new IllegalStateException("UTF-8 unsupported", e);
        }
    }
}
package com.kudi.coder.auditsdk;

import com.fasterxml.jackson.databind.ObjectMapper;
import com.fasterxml.jackson.datatype.jsr310.JavaTimeModule;
import com.kudi.coder.auditsdk.exception.AuditAuthException;
import com.kudi.coder.auditsdk.exception.AuditException;
import com.kudi.coder.auditsdk.exception.AuditRateLimitException;
import com.kudi.coder.auditsdk.model.AuditEvent;
import com.kudi.coder.auditsdk.model.AuditLog;
import com.kudi.coder.auditsdk.model.AuditLogQuery;
import com.kudi.coder.auditsdk.model.PageResult;
import okhttp3.mockwebserver.MockResponse;
import okhttp3.mockwebserver.MockWebServer;
import okhttp3.mockwebserver.RecordedRequest;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;

import java.time.Instant;
import java.util.Collections;
import java.util.List;
import java.util.concurrent.atomic.AtomicReference;

import static org.junit.jupiter.api.Assertions.*;

class AuditServiceClientTest {

    private MockWebServer server;
    private AuditServiceClient client;

    @BeforeEach
    void setUp() {
        server = new MockWebServer();
        try {
            server.start();
        } catch (Exception e) {
            throw new RuntimeException(e);
        }
        AtomicReference<String> bearer = new AtomicReference<>("test-token");
        client = new AuditServiceClient(server.url("/").toString(), bearer::get);
    }

    @AfterEach
    void tearDown() throws Exception {
        server.shutdown();
    }

    @Test
    void sendEvent_returns_log_on_200() throws Exception {
        String respJson = "{\"id\":\"log-1\",\"actor\":\"alice\",\"action\":\"TASK_CREATED\","
                + "\"resourceType\":\"task\",\"resourceId\":\"42\","
                + "\"payload\":{\"k\":\"v\"},\"occurredAt\":\"2026-06-29T10:00:00Z\"}";
        server.enqueue(new MockResponse().setResponseCode(200).setBody(respJson)
                .addHeader("Content-Type", "application/json"));

        AuditEvent ev = new AuditEvent("alice", "TASK_CREATED", "task", "42",
                Collections.singletonMap("k", "v"), Instant.parse("2026-06-29T10:00:00Z"));
        AuditLog log = client.sendEvent(ev);

        assertEquals("log-1", log.getId());
        assertEquals("alice", log.getActor());
        assertEquals("TASK_CREATED", log.getAction());

        RecordedRequest req = server.takeRequest();
        assertEquals("POST", req.getMethod());
        assertEquals("/v1/audit/events", req.getPath());
        assertEquals("Bearer test-token", req.getHeader("Authorization"));
    }

    @Test
    void queryLogs_parses_pagination() throws Exception {
        String respJson = "{\"items\":[{\"id\":\"a\",\"actor\":\"u1\",\"action\":\"X\"}],"
                + "\"total\":1,\"page\":1,\"size\":20}";
        server.enqueue(new MockResponse().setResponseCode(200).setBody(respJson)
                .addHeader("Content-Type", "application/json"));

        AuditLogQuery q = new AuditLogQuery();
        q.setActor("u1");
        PageResult<AuditLog> page = client.queryLogs(q);

        assertEquals(1, page.getTotal());
        assertEquals(1, page.getItems().size());
        assertEquals("a", page.getItems().get(0).getId());

        RecordedRequest req = server.takeRequest();
        assertTrue(req.getPath().startsWith("/v1/audit/logs?"));
        assertTrue(req.getPath().contains("actor=u1"));
    }

    @Test
    void sendEvent_throws_AuthException_on_401() {
        server.enqueue(new MockResponse().setResponseCode(401).setBody("unauthorized"));

        AuditEvent ev = new AuditEvent("a", "A", "t", "1", null, Instant.now());
        assertThrows(AuditAuthException.class, () -> client.sendEvent(ev));
    }

    @Test
    void sendEvent_throws_AuditException_on_500() {
        server.enqueue(new MockResponse().setResponseCode(500).setBody("server err"));

        AuditEvent ev = new AuditEvent("a", "A", "t", "1", null, Instant.now());
        assertThrows(AuditException.class, () -> client.sendEvent(ev));
    }
}
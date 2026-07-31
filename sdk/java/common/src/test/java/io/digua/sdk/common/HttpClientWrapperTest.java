package io.digua.common;

import io.digua.HttpClientWrapper;
import io.digua.HttpRequest;
import io.digua.HttpResponse;
import okhttp3.mockwebserver.MockResponse;
import okhttp3.mockwebserver.MockWebServer;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;

import java.io.IOException;

import static org.junit.jupiter.api.Assertions.assertEquals;

class HttpClientWrapperTest {
    private MockWebServer server;
    private HttpClientWrapper http;

    @BeforeEach
    void setUp() throws IOException {
        server = new MockWebServer();
        server.start();
        SdkConfig cfg = new SdkConfig(server.url("/").toString());
        cfg.setConnectTimeoutMs(2000);
        cfg.setReadTimeoutMs(2000);
        http = new HttpClientWrapper(cfg);
    }

    @AfterEach
    void tearDown() throws IOException {
        server.shutdown();
    }

    @Test
    void getReturnsBody() throws Exception {
        server.enqueue(new MockResponse().setResponseCode(200).setBody("hello"));
        HttpResponse r = http.execute(HttpRequest.get(server.url("/ping").toString()));
        assertEquals(200, r.getStatusCode());
        assertEquals("hello", r.getBody());
    }

    @Test
    void postWithJsonBody() throws Exception {
        server.enqueue(new MockResponse().setResponseCode(201).setBody("{\"ok\":true}"));
        HttpRequest req = HttpRequest.post(server.url("/v1/x").toString())
                .jsonBody(java.util.Collections.singletonMap("a", 1));
        HttpResponse r = http.execute(req);
        assertEquals(201, r.getStatusCode());
        okhttp3.mockwebserver.RecordedRequest recorded = server.takeRequest();
        assertEquals("POST", recorded.getMethod());
        assertEquals("{\"a\":1}", recorded.getBody().readUtf8());
        assertEquals("application/json", recorded.getHeader("Content-Type"));
    }
}

package com.kudi.coder.httpclient;

import okhttp3.mockwebserver.MockResponse;
import okhttp3.mockwebserver.MockWebServer;
import okhttp3.mockwebserver.RecordedRequest;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;
import static org.junit.jupiter.api.Assertions.*;

class KudiWebClientTest {
    private MockWebServer server;

    @BeforeEach
    void setUp() throws Exception {
        server = new MockWebServer();
        java.net.InetAddress ipv4 = java.net.InetAddress.getByName("127.0.0.1");
        server.start(ipv4, 0);
    }

    @AfterEach
    void tearDown() throws Exception {
        server.shutdown();
    }

    /** Build a URL pointing at the test server with 127.0.0.1 instead of localhost. */
    private String url(String path) {
        return "http://127.0.0.1:" + server.getPort() + path;
    }

    @Test
    void fetchesTokenOnceAndUsesForSubsequentCalls() throws Exception {
        server.enqueue(new MockResponse()
                .setResponseCode(200)
                .setBody("{\"access_token\":\"svc-token\",\"token_type\":\"Bearer\",\"expires_in\":3600}"));
        server.enqueue(new MockResponse().setResponseCode(200).setBody("{}"));
        server.enqueue(new MockResponse().setResponseCode(200).setBody("{}"));
        server.enqueue(new MockResponse().setResponseCode(200).setBody("{}"));

        var client = KudiWebClient.create()
                .tokenEndpoint(url("/api/tokens/access"))
                .withCredential("a", "s1");
        client.get(url("/api/x"), null);
        client.get(url("/api/y"), null);
        client.post(url("/api/z"), "{}", null);

        RecordedRequest tokenReq = server.takeRequest();
        assertEquals("/api/tokens/access", tokenReq.getPath());

        for (int i = 0; i < 3; i++) {
            RecordedRequest r = server.takeRequest();
            assertEquals("Bearer svc-token", r.getHeader("Authorization"));
        }
    }

    @Test
    void userBearerPassthrough() throws Exception {
        server.enqueue(new MockResponse().setResponseCode(200).setBody("{}"));
        var client = KudiWebClient.create()
                .tokenEndpoint(url("/api/tokens/access"))
                .withCredential("a", "s1");
        client.get(url("/api/me"), "user.jwt");
        RecordedRequest r = server.takeRequest();
        assertEquals("Bearer user.jwt", r.getHeader("Authorization"));
    }
}

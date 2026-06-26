package com.kudi.coder.httpclient;

import com.kudi.coder.httpclient.Exceptions.AuthException;
import com.kudi.coder.httpclient.Exceptions.RateLimitException;
import com.kudi.coder.httpclient.Exceptions.ServerException;
import com.kudi.coder.httpclient.Exceptions.ValidationException;
import okhttp3.mockwebserver.MockResponse;
import okhttp3.mockwebserver.MockWebServer;
import okhttp3.mockwebserver.RecordedRequest;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertNull;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

class KudiHttpClientTest {
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

    private String url(String path) {
        return "http://127.0.0.1:" + server.getPort() + path;
    }

    @Test
    void passesBearerThrough() throws Exception {
        server.enqueue(new MockResponse().setResponseCode(200).setBody("{\"id\":\"u-1\"}"));
        KudiHttpClient client = KudiHttpClient.create();

        HttpTransport.Response resp = client.get(url("/v1/iam/users/u-1"), "user.jwt");

        assertEquals(200, resp.statusCode);
        RecordedRequest r = server.takeRequest();
        assertEquals("/v1/iam/users/u-1", r.getPath());
        assertEquals("Bearer user.jwt", r.getHeader("Authorization"));
    }

    @Test
    void omitsAuthorizationWhenBearerNull() throws Exception {
        server.enqueue(new MockResponse().setResponseCode(200).setBody("{}"));
        KudiHttpClient client = KudiHttpClient.create();

        client.get(url("/v1/iam/health"), null);

        RecordedRequest r = server.takeRequest();
        assertNull(r.getHeader("Authorization"));
    }

    @Test
    void postSendsJsonContentType() throws Exception {
        server.enqueue(new MockResponse().setResponseCode(200).setBody("{\"id\":\"u-2\"}"));
        KudiHttpClient client = KudiHttpClient.create();

        client.post(url("/v1/iam/users"), "{\"username\":\"alice\"}", "user.jwt");

        RecordedRequest r = server.takeRequest();
        assertEquals("POST", r.getMethod());
        assertEquals("application/json", r.getHeader("Content-Type"));
        assertTrue(r.getBody().readUtf8().contains("\"username\":\"alice\""));
    }

    @Test
    void putAndDeleteVerbsRouted() throws Exception {
        server.enqueue(new MockResponse().setResponseCode(200).setBody("{}"));
        server.enqueue(new MockResponse().setResponseCode(204).setBody(""));
        KudiHttpClient client = KudiHttpClient.create();

        client.put(url("/v1/iam/users/u-1"), "{\"nickname\":\"A\"}", "user.jwt");
        client.delete(url("/v1/iam/users/u-1"), "user.jwt");

        assertEquals("PUT", server.takeRequest().getMethod());
        assertEquals("DELETE", server.takeRequest().getMethod());
    }

    @Test
    void mapsErrorStatusesToTypedExceptions() throws Exception {
        KudiHttpClient client = KudiHttpClient.create();
        KudiHttpOptions optsNoRetry = client.options();
        optsNoRetry.retryCount = 0;

        server.enqueue(new MockResponse().setResponseCode(401).setBody("{}"));
        assertThrows(AuthException.class, () -> client.get(url("/x"), null));

        server.enqueue(new MockResponse().setResponseCode(429).setBody("{}"));
        assertThrows(RateLimitException.class, () -> client.get(url("/x"), null));

        server.enqueue(new MockResponse().setResponseCode(422).setBody("{}"));
        assertThrows(ValidationException.class, () -> client.get(url("/x"), null));

        server.enqueue(new MockResponse().setResponseCode(500).setBody("{}"));
        assertThrows(ServerException.class, () -> client.get(url("/x"), null));
    }

    @Test
    void retriesOn5xx() throws Exception {
        server.enqueue(new MockResponse().setResponseCode(503).setBody(""));
        server.enqueue(new MockResponse().setResponseCode(200).setBody("{\"ok\":true}"));
        KudiHttpOptions opts = new KudiHttpOptions();
        opts.retryCount = 1;
        KudiHttpClient client = KudiHttpClient.create(opts);

        HttpTransport.Response resp = client.get(url("/v1/health"), null);

        assertEquals(200, resp.statusCode);
        assertEquals(2, server.getRequestCount());
    }
}

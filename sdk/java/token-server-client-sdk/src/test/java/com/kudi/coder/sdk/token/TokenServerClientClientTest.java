package com.kudi.coder.sdk.token;

import okhttp3.mockwebserver.MockResponse;
import okhttp3.mockwebserver.MockWebServer;
import okhttp3.mockwebserver.RecordedRequest;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;

import java.io.IOException;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

class TokenServerClientClientTest {
    private MockWebServer gw;
    private TokenServerClientClient client;

    @BeforeEach
    void setUp() throws IOException {
        gw = new MockWebServer();
        gw.start();

        gw.enqueue(new MockResponse()
                .setResponseCode(200)
                .setBody("{\"access_token\":\"tc\",\"refresh_token\":\"tr\",\"token_type\":\"Bearer\",\"expires_in\":300}"));

        client = TokenServerClientClient.create(gw.url("/").toString(), "ak", "sec", "code-1");
    }

    @AfterEach
    void tearDown() throws IOException { gw.shutdown(); }

    @Test
    void createApiKeyAfterLogin() throws Exception {
        gw.enqueue(new MockResponse()
                .setResponseCode(200)
                .setBody("{\"api_key\":\"ak-2\",\"secret\":\"sec-2\",\"created_at\":\"2026-06-08T00:00:00Z\"}"));

        ApiKeyInfo key = client.createApiKey("backend-x", "cid", "csec");
        assertEquals("ak-2", key.getApiKey());

        RecordedRequest login = gw.takeRequest();
        assertTrue(login.getPath().endsWith("/api/tokens/access"));

        RecordedRequest createKey = gw.takeRequest();
        assertEquals("POST", createKey.getMethod());
        assertTrue(createKey.getPath().endsWith("/api/api-keys"));
        String body = createKey.getBody().readUtf8();
        assertTrue(body.contains("name=backend-x"));
        assertTrue(body.contains("client_id=cid"));
    }
}

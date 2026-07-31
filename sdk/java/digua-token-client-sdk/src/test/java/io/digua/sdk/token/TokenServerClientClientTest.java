package io.digua.token;

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
                .setBody("{\"api_key_id\":\"ak-2\",\"api_key_secret\":\"sec-2\"}"));

        ApiKeyInfo key = client.createApiKey("backend-x",
                java.util.Arrays.asList("notification.admin"),
                java.util.Arrays.asList("client_credentials"));
        assertEquals("ak-2", key.getApiKeyId());
        assertEquals("sec-2", key.getApiKeySecret());

        RecordedRequest login = gw.takeRequest();
        assertTrue(login.getPath().endsWith("/v1/tokens/access"));
        assertEquals("application/json", login.getHeader("Content-Type"));

        RecordedRequest createKey = gw.takeRequest();
        assertEquals("POST", createKey.getMethod());
        assertTrue(createKey.getPath().endsWith("/v1/api-keys"));
        assertEquals("application/json", createKey.getHeader("Content-Type"));
        String body = createKey.getBody().readUtf8();
        assertTrue(body.contains("\"name\":\"backend-x\""));
        assertTrue(body.contains("\"permissions\":[\"notification.admin\"]"));
        assertTrue(body.contains("\"grant_types\":[\"client_credentials\"]"));
    }
}

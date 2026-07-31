package io.digua.token;

import io.digua.TokenInfo;
import okhttp3.mockwebserver.MockResponse;
import okhttp3.mockwebserver.MockWebServer;
import okhttp3.mockwebserver.RecordedRequest;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;

import java.io.IOException;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

class TokenServerUserClientTest {
    private MockWebServer gw;
    private TokenServerUserClient client;

    @BeforeEach
    void setUp() throws IOException {
        gw = new MockWebServer();
        gw.start();

        gw.enqueue(new MockResponse()
                .setResponseCode(200)
                .setBody("{\"access_token\":\"t-tok\",\"refresh_token\":\"t-ref\",\"token_type\":\"Bearer\",\"expires_in\":300}"));

        client = TokenServerUserClient.login(gw.url("/").toString(), "alice", "secret");
    }

    @AfterEach
    void tearDown() throws IOException { gw.shutdown(); }

    @Test
    void refreshExchangesRefreshToken() throws Exception {
        gw.enqueue(new MockResponse()
                .setResponseCode(200)
                .setBody("{\"access_token\":\"t-new\",\"refresh_token\":\"t-new-r\",\"token_type\":\"Bearer\",\"expires_in\":300}"));

        TokenInfo t = client.refresh("t-ref", "cid", "csec");
        assertEquals("t-new", t.getAccessToken());

        RecordedRequest login = gw.takeRequest();
        assertEquals("POST", login.getMethod());
        assertTrue(login.getPath().endsWith("/v1/iam/auth/login"));

        RecordedRequest refresh = gw.takeRequest();
        assertEquals("POST", refresh.getMethod());
        assertTrue(refresh.getPath().endsWith("/v1/tokens/refresh"));
        assertEquals("application/json", refresh.getHeader("Content-Type"));
        String body = refresh.getBody().readUtf8();
        assertTrue(body.contains("\"refresh_token\":\"t-ref\""));
        assertTrue(body.contains("\"client_id\":\"cid\""));
    }
}

package com.kudi.coder.sdk.user;

import okhttp3.mockwebserver.MockResponse;
import okhttp3.mockwebserver.MockWebServer;
import okhttp3.mockwebserver.RecordedRequest;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;

import java.io.IOException;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

class UserServiceClientClientTest {
    private MockWebServer gw;
    private UserServiceClientClient client;

    @BeforeEach
    void setUp() throws IOException {
        gw = new MockWebServer();
        gw.start();

        gw.enqueue(new MockResponse()
                .setResponseCode(200)
                .setBody("{\"access_token\":\"c-tok\",\"refresh_token\":\"c-ref\",\"token_type\":\"Bearer\",\"expires_in\":600}"));

        client = UserServiceClientClient.create(gw.url("/").toString(), "ak", "sec", "code-1");
    }

    @AfterEach
    void tearDown() throws IOException { gw.shutdown(); }

    @Test
    void getByIdUsesClientCredentials() throws Exception {
        gw.enqueue(new MockResponse()
                .setResponseCode(200)
                .setBody("{\"id\":\"u-9\"}"));

        User u = client.getById("u-9");
        assertEquals("u-9", u.getId());

        RecordedRequest login = gw.takeRequest();
        assertEquals("POST", login.getMethod());
        assertTrue(login.getPath().endsWith("/api/tokens/access"));
        String body = login.getBody().readUtf8();
        assertTrue(body.contains("grant_type=authorization_code"));
        assertTrue(body.contains("client_id=ak"));
        assertTrue(body.contains("code=code-1"));

        RecordedRequest userReq = gw.takeRequest();
        assertEquals("GET", userReq.getMethod());
        assertTrue(userReq.getPath().endsWith("/api/users/u-9"));
        assertEquals("Bearer c-tok", userReq.getHeader("Authorization"));
    }
}

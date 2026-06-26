package com.kudi.coder.sdk.user;

import okhttp3.mockwebserver.MockResponse;
import okhttp3.mockwebserver.MockWebServer;
import okhttp3.mockwebserver.RecordedRequest;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;

import java.io.IOException;
import java.util.List;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

class UserServiceUserClientTest {
    private MockWebServer gw;
    private UserServiceUserClient client;

    @BeforeEach
    void setUp() throws IOException {
        gw = new MockWebServer();
        gw.start();

        gw.enqueue(new MockResponse()
                .setResponseCode(200)
                .setBody("{\"access_token\":\"u-tok\",\"refresh_token\":\"u-ref\",\"token_type\":\"Bearer\",\"expires_in\":600}"));

        client = UserServiceUserClient.login(gw.url("/").toString(), "alice", "secret");
    }

    @AfterEach
    void tearDown() throws IOException {
        gw.shutdown();
    }

    @Test
    void getByIdSendsBearerAfterLogin() throws Exception {
        gw.enqueue(new MockResponse()
                .setResponseCode(200)
                .setBody("{\"id\":\"u-1\",\"username\":\"alice\",\"email\":\"a@x.com\"}"));

        User u = client.getById("u-1");
        assertEquals("u-1", u.getId());

        RecordedRequest login = gw.takeRequest();
        assertEquals("POST", login.getMethod());
        assertTrue(login.getPath().endsWith("/v1/iam/auth/login"));
        assertEquals("application/json", login.getHeader("Content-Type"));
        String loginBody = login.getBody().readUtf8();
        assertTrue(loginBody.contains("\"username\":\"alice\""));
        assertTrue(loginBody.contains("\"password\":\"secret\""));

        RecordedRequest userReq = gw.takeRequest();
        assertEquals("GET", userReq.getMethod());
        assertTrue(userReq.getPath().endsWith("/v1/iam/users/u-1"));
        assertEquals("Bearer u-tok", userReq.getHeader("Authorization"));
    }

    @Test
    void listAsyncReturnsUsers() throws Exception {
        gw.enqueue(new MockResponse()
                .setResponseCode(200)
                .setBody("[{\"id\":\"u-1\"},{\"id\":\"u-2\"}]"));

        List<User> users = client.listAsync().get();
        assertEquals(2, users.size());
        assertEquals("u-1", users.get(0).getId());
    }
}

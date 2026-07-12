package com.kudi.coder.filesdk;

import okhttp3.mockwebserver.MockResponse;
import okhttp3.mockwebserver.MockWebServer;
import okhttp3.mockwebserver.RecordedRequest;
import com.kudi.coder.filesdk.model.FileInfo;
import com.kudi.coder.filesdk.model.UploadFileMeta;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;

import java.nio.charset.StandardCharsets;
import java.util.concurrent.atomic.AtomicReference;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * Verifies multipart body shape: a JSON {@code metadata} part and a raw-bytes {@code file} part.
 */
class FileServiceClientUploadTest {

    private MockWebServer server;
    private FileServiceClient client;
    private AtomicReference<String> bearer;

    @BeforeEach
    void setUp() throws Exception {
        server = new MockWebServer();
        server.start();
        bearer = new AtomicReference<>("test-token");
        client = new FileServiceClient(server.url("/").toString(), bearer::get);
    }

    @AfterEach
    void tearDown() throws Exception {
        server.shutdown();
    }

    @Test
    void uploadFile_sends_multipart_with_metadata_and_bytes() throws Exception {
        server.enqueue(new MockResponse()
                .setResponseCode(200)
                .setBody("{\"id\":42,\"name\":\"req.md\",\"size\":5,\"contentType\":\"text/markdown\"}")
                .addHeader("Content-Type", "application/json"));

        UploadFileMeta meta = UploadFileMeta.builder()
                .name("req.md")
                .size(5)
                .contentType("text/markdown")
                .tenantId("acme")
                .tag("requirements")
                .build();
        byte[] content = "hello".getBytes(StandardCharsets.UTF_8);

        FileInfo info = client.uploadFile(meta, content);
        assertEquals(42L, info.getId());
        assertEquals("req.md", info.getName());

        RecordedRequest req = server.takeRequest();
        assertEquals("POST", req.getMethod());
        assertEquals("/v1/file/files/upload", req.getPath());

        String body = req.getBody().readUtf8();
        assertNotNull(body);
        // Boundary marker means we received a multipart payload
        assertTrue(body.contains("Content-Disposition: form-data; name=\"metadata\""),
                "metadata part should be present");
        assertTrue(body.contains("Content-Disposition: form-data; name=\"file\"; filename=\"req.md\""),
                "file part with filename should be present");
        assertTrue(body.contains("\"name\":\"req.md\""));
        assertTrue(body.contains("hello"));
        assertEquals("Bearer test-token", req.getHeader("Authorization"));
    }
}

package com.kudi.coder.filesdk;

import com.kudi.coder.filesdk.exception.FileException;
import com.kudi.coder.filesdk.model.FileInfo;
import com.kudi.coder.filesdk.model.FileQuery;
import com.kudi.coder.filesdk.model.PageResult;
import com.kudi.coder.filesdk.model.PresignedUpload;
import com.kudi.coder.filesdk.model.UploadFileMeta;
import okhttp3.mockwebserver.MockResponse;
import okhttp3.mockwebserver.MockWebServer;
import okhttp3.mockwebserver.RecordedRequest;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;

import java.time.Instant;
import java.util.concurrent.atomic.AtomicReference;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

class FileServiceClientTest {

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
    void getFileInfo_returns_info_on_200() throws Exception {
        server.enqueue(new MockResponse()
                .setResponseCode(200)
                .setBody("{\"id\":7,\"name\":\"spec.md\",\"size\":4096,\"contentType\":\"text/markdown\",\"sha256\":\"abc\",\"url\":\"https://files.example/7\",\"createdAt\":\"2026-07-11T10:00:00Z\"}")
                .addHeader("Content-Type", "application/json"));

        FileInfo info = client.getFileInfo(7L);
        assertEquals(7L, info.getId());
        assertEquals("spec.md", info.getName());
        assertEquals(4096L, info.getSize());
        assertEquals("abc", info.getSha256());

        RecordedRequest req = server.takeRequest();
        assertEquals("GET", req.getMethod());
        assertEquals("/v1/file/files/7", req.getPath());
        assertEquals("Bearer test-token", req.getHeader("Authorization"));
    }

    @Test
    void queryFiles_parses_pagination() throws Exception {
        server.enqueue(new MockResponse()
                .setResponseCode(200)
                .setBody("{\"items\":[{\"id\":1,\"name\":\"a.md\"}],\"total\":1,\"page\":1,\"size\":20}")
                .addHeader("Content-Type", "application/json"));

        FileQuery q = new FileQuery();
        q.setTenantId("acme");
        q.setTag("docs");
        PageResult<FileInfo> page = client.queryFiles(q);
        assertEquals(1L, page.getTotal());

        RecordedRequest req = server.takeRequest();
        assertTrue(req.getPath().startsWith("/v1/file/files?"));
        assertTrue(req.getPath().contains("tenant_id=acme"));
        assertTrue(req.getPath().contains("tag=docs"));
    }

    @Test
    void deleteFile_sends_DELETE() throws Exception {
        server.enqueue(new MockResponse().setResponseCode(200).setBody(""));
        client.deleteFile(3L);
        RecordedRequest req = server.takeRequest();
        assertEquals("DELETE", req.getMethod());
        assertEquals("/v1/file/files/3", req.getPath());
    }

    @Test
    void createPresignedUpload_parses_upload_url() throws Exception {
        server.enqueue(new MockResponse()
                .setResponseCode(200)
                .setBody("{\"fileId\":\"f-1\",\"uploadUrl\":\"https://files.example/upload/f-1\",\"method\":\"PUT\",\"expiresAt\":\"2026-07-11T11:00:00Z\"}")
                .addHeader("Content-Type", "application/json"));

        PresignedUpload p = client.createPresignedUpload(
                UploadFileMeta.builder().name("x.pdf").size(1024).contentType("application/pdf").build());
        assertEquals("f-1", p.getFileId());
        assertEquals("PUT", p.getMethod());

        RecordedRequest req = server.takeRequest();
        assertEquals("POST", req.getMethod());
        assertEquals("/v1/file/files/presign", req.getPath());
        assertTrue(req.getBody().readUtf8().contains("\"name\":\"x.pdf\""));
    }

    @Test
    void getFileInfo_throws_FileException_on_500() {
        server.enqueue(new MockResponse().setResponseCode(500).setBody("server err"));
        assertThrows(FileException.class, () -> client.getFileInfo(1L));
    }
}

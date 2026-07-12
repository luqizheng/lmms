package com.coder.pm.infra.service;

import com.kudi.coder.filesdk.FileServiceClient;
import com.kudi.coder.filesdk.model.FileInfo;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.extension.ExtendWith;
import org.mockito.Mock;
import org.mockito.junit.jupiter.MockitoExtension;

import static org.junit.jupiter.api.Assertions.*;
import static org.mockito.ArgumentMatchers.*;
import static org.mockito.Mockito.*;

@ExtendWith(MockitoExtension.class)
class AttachmentServiceTest {

    @Mock private FileServiceClient client;

    @Test
    void upload_delegatesToFileServiceClient() {
        FileInfo expected = new FileInfo();
        expected.setId(42L);
        expected.setName("test.txt");

        when(client.uploadFile(any(), any())).thenReturn(expected);

        AttachmentService svc = new AttachmentService(client);
        FileInfo result = svc.upload("test.txt", "hello".getBytes(), "kb:doc:1", "1");

        assertNotNull(result);
        assertEquals(42L, result.getId());
        assertEquals("test.txt", result.getName());
        verify(client).uploadFile(
                argThat(m -> "test.txt".equals(m.getName()) && "kb:doc:1".equals(m.getTag())),
                eq("hello".getBytes()));
    }
}

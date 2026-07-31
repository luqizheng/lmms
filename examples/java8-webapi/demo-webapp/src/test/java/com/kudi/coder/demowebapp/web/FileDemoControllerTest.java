package io.digua.web;

import io.digua.KudiCoderProperties;
import io.digua.WebMvcConfig;
import io.digua.JwtAuthInterceptor;
import io.digua.PermissionAspect;
import io.digua.HttpTransport;
import io.digua.KudiHttpClient;
import org.junit.jupiter.api.Test;
import org.mockito.ArgumentCaptor;
import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.boot.context.properties.EnableConfigurationProperties;
import org.springframework.boot.test.autoconfigure.web.servlet.WebMvcTest;
import org.springframework.boot.test.mock.mockito.MockBean;
import org.springframework.context.annotation.Import;
import org.springframework.mock.web.MockMultipartFile;
import org.springframework.test.web.servlet.MockMvc;

import static org.junit.jupiter.api.Assertions.assertTrue;
import static org.mockito.ArgumentMatchers.any;
import static org.mockito.Mockito.verify;
import static org.mockito.Mockito.when;
import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.get;
import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.multipart;
import static org.springframework.test.web.servlet.result.MockMvcResultMatchers.content;
import static org.springframework.test.web.servlet.result.MockMvcResultMatchers.jsonPath;
import static org.springframework.test.web.servlet.result.MockMvcResultMatchers.status;

@WebMvcTest(FileDemoController.class)
@Import({JwtAuthInterceptor.class, PermissionAspect.class, WebMvcConfig.class})
@EnableConfigurationProperties(KudiCoderProperties.class)
class FileDemoControllerTest {

    @Autowired private MockMvc mvc;
    @MockBean private KudiHttpClient kudiHttpClient;

    @Test
    void upload_returns_file_id() throws Exception {
        HttpTransport.Response upstream = new HttpTransport.Response(200, "{\"id\":\"f-1\",\"size\":12}");
        when(kudiHttpClient.post(any(String.class), any(String.class), any())).thenReturn(upstream);

        byte[] body = "hello world!".getBytes();
        mvc.perform(multipart("/demo/file/upload")
                        .file(new MockMultipartFile("file", "test.txt", "text/plain", body))
                        .header("X-User-Id", "alice"))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.code").value("OK"))
                .andExpect(jsonPath("$.data.fileId").value("f-1"));

        ArgumentCaptor<String> urlCap = ArgumentCaptor.forClass(String.class);
        ArgumentCaptor<String> jsonCap = ArgumentCaptor.forClass(String.class);
        verify(kudiHttpClient).post(urlCap.capture(), jsonCap.capture(), any());
        assertTrue(urlCap.getValue().endsWith("/v1/file/files"),
                "URL should target /v1/file/files, got: " + urlCap.getValue());
        String sent = jsonCap.getValue();
        assertTrue(sent.contains("\"content_base64\":\""),
                "body missing content_base64 field: " + sent);
        assertTrue(sent.contains("aGVsbG8gd29ybGQh"),
                "body missing base64 of 'hello world!': " + sent);
    }

    @Test
    void download_returns_bytes() throws Exception {
        HttpTransport.Response upstream = new HttpTransport.Response(200, "hello");
        when(kudiHttpClient.get(any(String.class), any())).thenReturn(upstream);

        mvc.perform(get("/demo/file/f-1").header("X-User-Id", "alice"))
                .andExpect(status().isOk())
                .andExpect(content().bytes("hello".getBytes()));

        ArgumentCaptor<String> urlCap = ArgumentCaptor.forClass(String.class);
        verify(kudiHttpClient).get(urlCap.capture(), any());
        assertTrue(urlCap.getValue().endsWith("/v1/file/files/f-1"),
                "URL should target /v1/file/files/f-1, got: " + urlCap.getValue());
    }
}

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
import org.springframework.test.web.servlet.MockMvc;

import static org.junit.jupiter.api.Assertions.assertTrue;
import static org.mockito.ArgumentMatchers.any;
import static org.mockito.ArgumentMatchers.eq;
import static org.mockito.Mockito.verify;
import static org.mockito.Mockito.when;
import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.post;
import static org.springframework.test.web.servlet.result.MockMvcResultMatchers.jsonPath;
import static org.springframework.test.web.servlet.result.MockMvcResultMatchers.status;

@WebMvcTest(NotificationDemoController.class)
@Import({JwtAuthInterceptor.class, PermissionAspect.class, WebMvcConfig.class})
@EnableConfigurationProperties(KudiCoderProperties.class)
class NotificationDemoControllerTest {

    @Autowired private MockMvc mvc;
    @MockBean private KudiHttpClient kudiHttpClient;

    @Test
    void taskDone_sends_notification() throws Exception {
        HttpTransport.Response upstream = new HttpTransport.Response(200, "{\"id\":\"n-1\"}");
        when(kudiHttpClient.post(any(String.class), any(String.class), any())).thenReturn(upstream);

        String body = "{\"userId\":\"1001\",\"taskId\":\"42\",\"title\":\"Task 42 is done\"}";
        mvc.perform(post("/demo/notify/task-done")
                        .header("X-User-Id", "alice")
                        .contentType("application/json")
                        .content(body))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.code").value("OK"))
                .andExpect(jsonPath("$.data.accepted").value(true));

        ArgumentCaptor<String> urlCap = ArgumentCaptor.forClass(String.class);
        ArgumentCaptor<String> bodyCap = ArgumentCaptor.forClass(String.class);
        verify(kudiHttpClient).post(urlCap.capture(), bodyCap.capture(), eq(null));
        assertTrue(urlCap.getValue().endsWith("/v1/ntf/notifications"),
                "URL should target /v1/ntf/notifications, got: " + urlCap.getValue());
        String sent = bodyCap.getValue();
        assertTrue(sent.contains("\"user_id\":\"1001\""), "body missing user_id: " + sent);
        assertTrue(sent.contains("\"template\":\"TASK_DONE\""), "body missing template: " + sent);
        assertTrue(sent.contains("\"channel\":\"in_app\""), "body missing channel: " + sent);
    }
}
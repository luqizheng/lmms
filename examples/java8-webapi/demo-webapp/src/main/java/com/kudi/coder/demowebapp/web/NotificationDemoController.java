package com.kudi.coder.demowebapp.web;

import com.fasterxml.jackson.databind.ObjectMapper;
import com.kudi.coder.demo.shared.ApiResponse;
import com.kudi.coder.demowebapp.config.KudiCoderProperties;
import com.kudi.coder.httpclient.HttpTransport;
import com.kudi.coder.httpclient.KudiHttpClient;
import org.springframework.web.bind.annotation.PostMapping;
import org.springframework.web.bind.annotation.RequestBody;
import org.springframework.web.bind.annotation.RequestMapping;
import org.springframework.web.bind.annotation.RestController;

import java.util.Collections;
import java.util.HashMap;
import java.util.Map;

@RestController
@RequestMapping("/demo/notify")
public class NotificationDemoController {

    private final KudiHttpClient kudiHttpClient;
    private final KudiCoderProperties properties;
    private final ObjectMapper objectMapper;

    public NotificationDemoController(KudiHttpClient kudiHttpClient,
                                     KudiCoderProperties properties,
                                     ObjectMapper objectMapper) {
        this.kudiHttpClient = kudiHttpClient;
        this.properties = properties;
        this.objectMapper = objectMapper;
    }

    @PostMapping("/task-done")
    public ApiResponse<Map<String, Object>> taskDone(@RequestBody Map<String, String> body) throws Exception {
        String userId = body.get("userId");
        String taskId = body.get("taskId");
        String title = body.get("title");

        Map<String, Object> payload = new HashMap<>();
        payload.put("user_id", userId);
        payload.put("task_id", taskId);
        payload.put("title", title);

        Map<String, Object> body0 = new HashMap<>();
        body0.put("channel", "in_app");
        body0.put("user_id", userId);
        body0.put("template", "TASK_DONE");
        body0.put("payload", payload);
        String json = objectMapper.writeValueAsString(body0);

        String url = properties.getGateway().getBaseUrl() + "/v1/ntf/notifications";
        HttpTransport.Response resp = kudiHttpClient.post(url, json, null);

        return ApiResponse.ok(Collections.<String, Object>singletonMap("accepted", Boolean.TRUE));
    }
}
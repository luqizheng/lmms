package com.kudi.coder.demowebapp.web;

import com.kudi.coder.demo.shared.ApiResponse;
import com.kudi.coder.demowebapp.config.KudiCoderProperties;
import com.kudi.coder.httpclient.HttpTransport;
import com.kudi.coder.httpclient.KudiHttpClient;
import org.springframework.web.bind.annotation.PostMapping;
import org.springframework.web.bind.annotation.RequestBody;
import org.springframework.web.bind.annotation.RequestMapping;
import org.springframework.web.bind.annotation.RestController;

import java.util.Collections;
import java.util.Map;

@RestController
@RequestMapping("/demo/notify")
public class NotificationDemoController {

    private final KudiHttpClient kudiHttpClient;
    private final KudiCoderProperties properties;

    public NotificationDemoController(KudiHttpClient kudiHttpClient, KudiCoderProperties properties) {
        this.kudiHttpClient = kudiHttpClient;
        this.properties = properties;
    }

    @PostMapping("/task-done")
    public ApiResponse<Map<String, Object>> taskDone(@RequestBody Map<String, String> body) {
        String userId = body.get("userId");
        String taskId = body.get("taskId");
        String title = body.get("title");

        String payloadJson = "{\"user_id\":\"" + userId
                + "\",\"task_id\":\"" + taskId
                + "\",\"title\":\"" + title + "\"}";
        String json = "{\"channel\":\"in_app\","
                + "\"user_id\":\"" + userId + "\","
                + "\"template\":\"TASK_DONE\","
                + "\"payload\":" + payloadJson + "}";

        String url = properties.getGateway().getBaseUrl() + "/v1/ntf/notifications";
        HttpTransport.Response resp = kudiHttpClient.post(url, json, null);

        return ApiResponse.ok(Collections.<String, Object>singletonMap("accepted", Boolean.TRUE));
    }
}
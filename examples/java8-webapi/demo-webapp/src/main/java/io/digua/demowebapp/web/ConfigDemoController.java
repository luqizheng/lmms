package io.digua.web;

import io.digua.ApiResponse;
import io.digua.ConfigPoller;
import org.springframework.web.bind.annotation.GetMapping;
import org.springframework.web.bind.annotation.RequestMapping;
import org.springframework.web.bind.annotation.RequestParam;
import org.springframework.web.bind.annotation.RestController;

import java.util.LinkedHashMap;
import java.util.Map;

@RestController
@RequestMapping("/demo/config")
public class ConfigDemoController {

    private final ConfigPoller poller;

    public ConfigDemoController(ConfigPoller poller) {
        this.poller = poller;
    }

    @GetMapping("/show")
    public ApiResponse<Map<String, String>> show(@RequestParam String key) {
        Map<String, String> data = new LinkedHashMap<>();
        data.put("key", key);
        data.put("value", poller.get(key));
        return ApiResponse.ok(data);
    }

    @GetMapping("/wait-change")
    public ApiResponse<Map<String, Object>> waitChange(@RequestParam String key,
                                                       @RequestParam String expected,
                                                       @RequestParam(defaultValue = "30") long timeoutSec)
            throws InterruptedException {
        boolean changed = poller.waitForChange(key, expected, timeoutSec);
        Map<String, Object> data = new LinkedHashMap<>();
        data.put("changed", changed);
        data.put("current", poller.get(key));
        return ApiResponse.ok(data);
    }
}

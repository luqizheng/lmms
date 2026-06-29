package com.kudi.coder.demowebapp.web;

import com.fasterxml.jackson.databind.ObjectMapper;
import com.kudi.coder.demo.shared.ApiResponse;
import com.kudi.coder.demowebapp.config.KudiCoderProperties;
import com.kudi.coder.httpclient.HttpTransport;
import com.kudi.coder.httpclient.KudiHttpClient;
import org.springframework.http.HttpHeaders;
import org.springframework.http.HttpStatus;
import org.springframework.http.MediaType;
import org.springframework.http.ResponseEntity;
import org.springframework.web.bind.annotation.GetMapping;
import org.springframework.web.bind.annotation.PathVariable;
import org.springframework.web.bind.annotation.PostMapping;
import org.springframework.web.bind.annotation.RequestMapping;
import org.springframework.web.bind.annotation.RequestParam;
import org.springframework.web.bind.annotation.RestController;
import org.springframework.web.multipart.MultipartFile;

import java.nio.charset.StandardCharsets;
import java.util.Base64;
import java.util.HashMap;
import java.util.Map;

@RestController
@RequestMapping("/demo/file")
public class FileDemoController {

    private final KudiHttpClient kudiHttpClient;
    private final KudiCoderProperties properties;
    private final ObjectMapper objectMapper;

    public FileDemoController(KudiHttpClient kudiHttpClient,
                              KudiCoderProperties properties,
                              ObjectMapper objectMapper) {
        this.kudiHttpClient = kudiHttpClient;
        this.properties = properties;
        this.objectMapper = objectMapper;
    }

    @PostMapping("/upload")
    public ApiResponse<Map<String, Object>> upload(@RequestParam("file") MultipartFile file) throws Exception {
        if (file.isEmpty()) {
            throw new IllegalArgumentException("file is required");
        }
        String base64 = Base64.getEncoder().encodeToString(file.getBytes());
        String json = "{\"name\":\"" + escape(file.getOriginalFilename()) + "\","
                + "\"content_type\":\"" + escape(file.getContentType()) + "\","
                + "\"content_base64\":\"" + base64 + "\"}";
        String url = properties.getGateway().getBaseUrl() + "/v1/file/files";
        HttpTransport.Response resp = kudiHttpClient.post(url, json, null);
        String fileId = objectMapper.readTree(resp.body).path("id").asText("demo");

        Map<String, Object> data = new HashMap<>();
        data.put("fileId", fileId);
        data.put("name", file.getOriginalFilename());
        data.put("size", file.getSize());
        return ApiResponse.ok(data);
    }

    @GetMapping("/{id}")
    public ResponseEntity<byte[]> download(@PathVariable String id) {
        String url = properties.getGateway().getBaseUrl() + "/v1/file/files/" + id;
        HttpTransport.Response resp = kudiHttpClient.get(url, null);
        byte[] bytes = resp.body == null ? new byte[0] : resp.body.getBytes(StandardCharsets.UTF_8);

        HttpHeaders headers = new HttpHeaders();
        headers.setContentType(MediaType.APPLICATION_OCTET_STREAM);
        headers.setContentDispositionFormData("attachment", id);
        return new ResponseEntity<>(bytes, headers, HttpStatus.OK);
    }

    private static String escape(String s) {
        return s == null ? "" : s.replace("\\", "\\\\").replace("\"", "\\\"");
    }
}

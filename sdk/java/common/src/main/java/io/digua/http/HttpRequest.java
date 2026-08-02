package io.digua.http;

import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.Map;

public class HttpRequest {
    private String method;
    private String url;
    private Map<String, String> headers = new LinkedHashMap<>();
    private String body;

    public HttpRequest() {}

    public HttpRequest(String method, String url) {
        this.method = method;
        this.url = url;
    }

    public static HttpRequest get(String url) { return new HttpRequest("GET", url); }
    public static HttpRequest post(String url) { return new HttpRequest("POST", url); }
    public static HttpRequest put(String url) { return new HttpRequest("PUT", url); }
    public static HttpRequest delete(String url) { return new HttpRequest("DELETE", url); }
    public static HttpRequest patch(String url) { return new HttpRequest("PATCH", url); }

    public HttpRequest header(String name, String value) {
        this.headers.put(name, value);
        return this;
    }

    public HttpRequest bearer(String token) {
        return header("Authorization", "Bearer " + token);
    }

    public HttpRequest jsonBody(Object value) {
        try {
            this.body = io.digua.common.JsonUtil.toJson(value);
        } catch (Exception e) {
            throw new RuntimeException(e);
        }
        return header("Content-Type", "application/json");
    }

    public HttpRequest body(String body, String contentType) {
        this.body = body;
        return header("Content-Type", contentType);
    }

    public String getMethod() { return method; }
    public String getUrl() { return url; }
    public Map<String, String> getHeaders() { return Collections.unmodifiableMap(headers); }
    public String getBody() { return body; }
}

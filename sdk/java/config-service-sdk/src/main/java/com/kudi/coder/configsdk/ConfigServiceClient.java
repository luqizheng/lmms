package com.kudi.coder.configsdk;

import com.fasterxml.jackson.core.type.TypeReference;
import com.fasterxml.jackson.databind.DeserializationFeature;
import com.fasterxml.jackson.databind.ObjectMapper;
import com.fasterxml.jackson.datatype.jsr310.JavaTimeModule;
import com.kudi.coder.configsdk.exception.ConfigAuthException;
import com.kudi.coder.configsdk.exception.ConfigException;
import com.kudi.coder.configsdk.exception.ConfigRateLimitException;
import com.kudi.coder.configsdk.model.ConfigItem;
import com.kudi.coder.configsdk.model.ConfigItemQuery;
import com.kudi.coder.configsdk.model.ConfigItemRequest;
import com.kudi.coder.configsdk.model.ConfigItemUpdate;
import com.kudi.coder.configsdk.model.Namespace;
import com.kudi.coder.configsdk.model.NamespaceQuery;
import com.kudi.coder.configsdk.model.NamespaceRequest;
import com.kudi.coder.configsdk.model.PageResult;
import com.kudi.coder.configsdk.model.WatchCallback;
import okhttp3.MediaType;
import okhttp3.OkHttpClient;
import okhttp3.Request;
import okhttp3.RequestBody;
import okhttp3.Response;
import okhttp3.ResponseBody;

import java.io.IOException;
import java.util.concurrent.TimeUnit;
import java.util.function.Supplier;

/**
 * Java 8 facade for config-service. Covers MVP CRUD on namespaces + items +
 * namespace lookup, plus a no-op watch stub.
 */
public class ConfigServiceClient {

    private static final MediaType JSON = MediaType.parse("application/json; charset=utf-8");
    private static final TypeReference<PageResult<Namespace>> NS_PAGE_TYPE =
            new TypeReference<PageResult<Namespace>>() {};
    private static final TypeReference<PageResult<ConfigItem>> ITEM_PAGE_TYPE =
            new TypeReference<PageResult<ConfigItem>>() {};

    private final String baseUrl;
    private final Supplier<String> bearerSupplier;
    private final OkHttpClient http;
    private final ObjectMapper mapper;

    public ConfigServiceClient(String baseUrl, Supplier<String> bearerSupplier) {
        this(baseUrl, bearerSupplier, 5000);
    }

    public ConfigServiceClient(String baseUrl, Supplier<String> bearerSupplier, int timeoutMs) {
        this.baseUrl = baseUrl.endsWith("/") ? baseUrl.substring(0, baseUrl.length() - 1) : baseUrl;
        this.bearerSupplier = bearerSupplier;
        this.http = new OkHttpClient.Builder()
                .connectTimeout(timeoutMs, TimeUnit.MILLISECONDS)
                .readTimeout(timeoutMs, TimeUnit.MILLISECONDS)
                .writeTimeout(timeoutMs, TimeUnit.MILLISECONDS)
                .build();
        this.mapper = new ObjectMapper()
                .registerModule(new JavaTimeModule())
                .configure(DeserializationFeature.FAIL_ON_UNKNOWN_PROPERTIES, false);
    }

    public Namespace createNamespace(NamespaceRequest req) {
        return execute("POST", "/v1/config/namespaces", mapper.valueToTree(req).toString(), Namespace.class);
    }

    public PageResult<Namespace> queryNamespaces(NamespaceQuery q) {
        StringBuilder url = new StringBuilder("/v1/config/namespaces?");
        if (q.getApp() != null) url.append("app=").append(encode(q.getApp())).append('&');
        url.append("page=").append(q.getPage()).append("&size=").append(q.getSize());
        try {
            return getPage(url.toString(), NS_PAGE_TYPE);
        } catch (IOException e) {
            throw new ConfigException("config queryNamespaces IO error: " + e.getMessage(), 0, e);
        }
    }

    public Namespace getNamespace(long id) {
        return execute("GET", "/v1/config/namespaces/" + id, null, Namespace.class);
    }

    public ConfigItem createConfigItem(long namespaceId, ConfigItemRequest req) {
        return execute("POST", "/v1/config/namespaces/" + namespaceId + "/items",
                mapper.valueToTree(req).toString(), ConfigItem.class);
    }

    public PageResult<ConfigItem> queryConfigItems(long namespaceId, ConfigItemQuery q) {
        StringBuilder url = new StringBuilder("/v1/config/namespaces/" + namespaceId + "/items?");
        if (q.getKey() != null) url.append("key=").append(encode(q.getKey())).append('&');
        url.append("page=").append(q.getPage()).append("&size=").append(q.getSize());
        try {
            return getPage(url.toString(), ITEM_PAGE_TYPE);
        } catch (IOException e) {
            throw new ConfigException("config queryConfigItems IO error: " + e.getMessage(), 0, e);
        }
    }

    public ConfigItem getConfigItem(long id) {
        return execute("GET", "/v1/config/items/" + id, null, ConfigItem.class);
    }

    public ConfigItem updateConfigItem(long id, ConfigItemUpdate req) {
        return execute("PUT", "/v1/config/items/" + id,
                mapper.valueToTree(req).toString(), ConfigItem.class);
    }

    public void deleteConfigItem(long id) {
        execute("DELETE", "/v1/config/items/" + id, null, Void.class);
    }

    /**
     * Watch stub — config-service exposes SSE on {@code /v1/config/namespaces/{id}/watch}, but
     * SSE streaming requires a long-lived connection and is non-trivial to implement correctly
     * across proxies. MVP throws {@link UnsupportedOperationException}; long-poll via
     * {@link #queryConfigItems(long, ConfigItemQuery)} or watch via RabbitMQ events instead.
     */
    public void watchNamespace(long namespaceId, WatchCallback cb) {
        throw new UnsupportedOperationException(
                "watchNamespace: SSE streaming is intentionally not implemented in MVP. " +
                        "Use queryConfigItems for long-poll or wire RabbitMQ events instead.");
    }

    private <T> T execute(String method, String path, String body, Class<T> respType) {
        try {
            Request.Builder b = baseRequest(path);
            RequestBody reqBody = (body == null) ? null : RequestBody.create(body, JSON);
            switch (method) {
                case "GET": b.get(); break;
                case "DELETE": b.delete(reqBody); break;
                case "POST": b.post(reqBody); break;
                case "PUT": b.put(reqBody); break;
                default: throw new IllegalArgumentException("Unsupported method: " + method);
            }
            try (Response resp = http.newCall(b.build()).execute()) {
                String respBody = readBody(resp);
                ensureSuccess(resp, respBody);
                if (respType == Void.class || respBody.isEmpty()) return null;
                return mapper.readValue(respBody, respType);
            }
        } catch (IOException e) {
            throw new ConfigException("config " + method + " " + path + " IO error: " + e.getMessage(), 0, e);
        }
    }

    private <T> PageResult<T> getPage(String path, TypeReference<PageResult<T>> type) throws IOException {
        Request req = baseRequest(path).get().build();
        try (Response resp = http.newCall(req).execute()) {
            String respBody = readBody(resp);
            ensureSuccess(resp, respBody);
            return mapper.readValue(respBody, type);
        }
    }

    private Request.Builder baseRequest(String path) {
        Request.Builder b = new Request.Builder().url(baseUrl + path);
        String bearer = bearerSupplier == null ? null : bearerSupplier.get();
        if (bearer != null && !bearer.isEmpty()) {
            b.header("Authorization", "Bearer " + bearer);
        }
        b.header("Accept", "application/json");
        return b;
    }

    private void ensureSuccess(Response resp, String body) {
        int code = resp.code();
        if (code >= 200 && code < 300) return;
        if (code == 401) throw new ConfigAuthException("config 401: " + body);
        if (code == 429) throw new ConfigRateLimitException("config 429: " + body);
        throw new ConfigException("config " + code + ": " + body, code);
    }

    private String readBody(Response resp) throws IOException {
        ResponseBody b = resp.body();
        return b == null ? "" : b.string();
    }

    private static String encode(String s) {
        try {
            return java.net.URLEncoder.encode(s, java.nio.charset.StandardCharsets.UTF_8.name());
        } catch (java.io.UnsupportedEncodingException e) {
            throw new IllegalStateException("UTF-8 is a JVM-mandatory charset", e);
        }
    }
}

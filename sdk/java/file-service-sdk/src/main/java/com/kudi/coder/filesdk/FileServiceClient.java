package com.kudi.coder.filesdk;

import com.fasterxml.jackson.core.type.TypeReference;
import com.fasterxml.jackson.databind.DeserializationFeature;
import com.fasterxml.jackson.databind.ObjectMapper;
import com.fasterxml.jackson.datatype.jsr310.JavaTimeModule;
import com.kudi.coder.filesdk.exception.FileAuthException;
import com.kudi.coder.filesdk.exception.FileException;
import com.kudi.coder.filesdk.exception.FileRateLimitException;
import com.kudi.coder.filesdk.model.FileInfo;
import com.kudi.coder.filesdk.model.FileQuery;
import com.kudi.coder.filesdk.model.PageResult;
import com.kudi.coder.filesdk.model.PresignedUpload;
import com.kudi.coder.filesdk.model.UploadFileMeta;
import okhttp3.MediaType;
import okhttp3.MultipartBody;
import okhttp3.OkHttpClient;
import okhttp3.Request;
import okhttp3.RequestBody;
import okhttp3.Response;
import okhttp3.ResponseBody;

import java.io.IOException;
import java.util.concurrent.TimeUnit;
import java.util.function.Supplier;

/**
 * Java 8 facade for file-service.
 *
 * <p>Covers MVP endpoints used by apps/pm for requirement / knowledge-base attachments.
 *
 * <p><strong>Multipart upload</strong> uses OkHttp {@link MultipartBody.Builder} with two parts:
 * {@code metadata} (JSON {@link UploadFileMeta}) + {@code file} (raw bytes). The metadata part
 * is JSON-serialized and posted with {@code Content-Type: application/json}; the file part is
 * posted as {@code application/octet-stream} with the original filename.
 */
public class FileServiceClient {

    private static final MediaType JSON = MediaType.parse("application/json; charset=utf-8");
    private static final MediaType OCTET = MediaType.parse("application/octet-stream");
    private static final TypeReference<PageResult<FileInfo>> PAGE_TYPE =
            new TypeReference<PageResult<FileInfo>>() {};

    private final String baseUrl;
    private final Supplier<String> bearerSupplier;
    private final OkHttpClient http;
    private final ObjectMapper mapper;

    public FileServiceClient(String baseUrl, Supplier<String> bearerSupplier) {
        this(baseUrl, bearerSupplier, 10000);
    }

    public FileServiceClient(String baseUrl, Supplier<String> bearerSupplier, int timeoutMs) {
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

    public PresignedUpload createPresignedUpload(UploadFileMeta meta) {
        try {
            String body = mapper.writeValueAsString(meta);
            Request req = baseRequest("/v1/file/files/presign")
                    .post(RequestBody.create(body, JSON))
                    .build();
            try (Response resp = http.newCall(req).execute()) {
                String respBody = readBody(resp);
                ensureSuccess(resp, respBody);
                return mapper.readValue(respBody, PresignedUpload.class);
            }
        } catch (IOException e) {
            throw new FileException("file presign IO error: " + e.getMessage(), 0, e);
        }
    }

    /**
     * One-shot multipart upload: metadata + raw file bytes in a single HTTP POST.
     */
    public FileInfo uploadFile(UploadFileMeta meta, byte[] content) {
        try {
        String metaJson = mapper.writeValueAsString(meta);
        MultipartBody body = new MultipartBody.Builder()
                .setType(MultipartBody.FORM)
                .addFormDataPart("metadata", null,
                        RequestBody.create(metaJson, MediaType.parse("application/json; charset=utf-8")))
                .addFormDataPart("file", meta.getName(),
                        RequestBody.create(content, OCTET))
                .build();

            Request req = baseRequest("/v1/file/files/upload")
                    .post(body)
                    .build();
            try (Response resp = http.newCall(req).execute()) {
                String respBody = readBody(resp);
                ensureSuccess(resp, respBody);
                return mapper.readValue(respBody, FileInfo.class);
            }
        } catch (IOException e) {
            throw new FileException("file upload IO error: " + e.getMessage(), 0, e);
        }
    }

    public FileInfo getFileInfo(long id) {
        try {
            Request req = baseRequest("/v1/file/files/" + id).get().build();
            try (Response resp = http.newCall(req).execute()) {
                String respBody = readBody(resp);
                ensureSuccess(resp, respBody);
                return mapper.readValue(respBody, FileInfo.class);
            }
        } catch (IOException e) {
            throw new FileException("file getFileInfo IO error: " + e.getMessage(), 0, e);
        }
    }

    public void deleteFile(long id) {
        try {
            Request req = baseRequest("/v1/file/files/" + id).delete().build();
            try (Response resp = http.newCall(req).execute()) {
                String respBody = readBody(resp);
                ensureSuccess(resp, respBody);
            }
        } catch (IOException e) {
            throw new FileException("file deleteFile IO error: " + e.getMessage(), 0, e);
        }
    }

    public PageResult<FileInfo> queryFiles(FileQuery q) {
        StringBuilder url = new StringBuilder("/v1/file/files?");
        if (q.getTenantId() != null) url.append("tenant_id=").append(encode(q.getTenantId())).append('&');
        if (q.getTag() != null) url.append("tag=").append(encode(q.getTag())).append('&');
        url.append("page=").append(q.getPage()).append("&size=").append(q.getSize());
        try {
            Request req = baseRequest(url.toString()).get().build();
            try (Response resp = http.newCall(req).execute()) {
                String respBody = readBody(resp);
                ensureSuccess(resp, respBody);
                return mapper.readValue(respBody, PAGE_TYPE);
            }
        } catch (IOException e) {
            throw new FileException("file queryFiles IO error: " + e.getMessage(), 0, e);
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
        if (code == 401) throw new FileAuthException("file 401: " + body);
        if (code == 429) throw new FileRateLimitException("file 429: " + body);
        throw new FileException("file " + code + ": " + body, code);
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

package io.digua.http;

import io.digua.SdkConfig;
import io.digua.NetworkException;

import java.io.BufferedReader;
import java.io.IOException;
import java.io.InputStream;
import java.io.InputStreamReader;
import java.io.OutputStream;
import java.net.HttpURLConnection;
import java.net.URI;
import java.net.URL;
import java.nio.charset.StandardCharsets;
import java.util.Map;

public class HttpClientWrapper {
    private final SdkConfig config;

    public HttpClientWrapper(SdkConfig config) {
        this.config = config;
    }

    public HttpResponse execute(HttpRequest request) {
        try {
            URL url = URI.create(request.getUrl()).toURL();
            HttpURLConnection conn = (HttpURLConnection) url.openConnection();
            conn.setRequestMethod(request.getMethod());
            conn.setConnectTimeout(config.getConnectTimeoutMs());
            conn.setReadTimeout(config.getReadTimeoutMs());
            conn.setUseCaches(false);
            conn.setRequestProperty("User-Agent", config.getUserAgent());
            conn.setRequestProperty("Accept", "application/json");
            for (Map.Entry<String, String> e : request.getHeaders().entrySet()) {
                conn.setRequestProperty(e.getKey(), e.getValue());
            }
            if (request.getBody() != null) {
                conn.setDoOutput(true);
                byte[] bytes = request.getBody().getBytes(StandardCharsets.UTF_8);
                conn.setFixedLengthStreamingMode(bytes.length);
                try (OutputStream os = conn.getOutputStream()) {
                    os.write(bytes);
                }
            }
            int code = conn.getResponseCode();
            String body = readBody(conn, code);
            return new HttpResponse(code, body);
        } catch (IOException e) {
            throw new NetworkException("HTTP request failed: " + e.getMessage(), e);
        }
    }

    private String readBody(HttpURLConnection conn, int code) throws IOException {
        InputStream is = (code >= 200 && code < 300) ? conn.getInputStream() : conn.getErrorStream();
        if (is == null) return "";
        try (BufferedReader br = new BufferedReader(new InputStreamReader(is, StandardCharsets.UTF_8))) {
            StringBuilder sb = new StringBuilder();
            String line;
            while ((line = br.readLine()) != null) {
                sb.append(line);
            }
            return sb.toString();
        }
    }
}

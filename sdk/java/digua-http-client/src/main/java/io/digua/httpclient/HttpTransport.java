package io.digua.httpclient;

import java.io.BufferedReader;
import java.io.DataOutputStream;
import java.io.IOException;
import java.io.InputStream;
import java.io.InputStreamReader;
import java.net.HttpURLConnection;
import java.net.URI;
import java.net.URL;
import java.nio.charset.StandardCharsets;
import io.digua.NetworkException;

public class HttpTransport {

    public static class Response {
        public final int statusCode;
        public final String body;

        public Response(int s, String b) {
            statusCode = s;
            body = b;
        }
    }

    private final KudiHttpOptions options;

    public HttpTransport(KudiHttpOptions options) {
        this.options = options;
    }

    public Response send(String method, String url, String bearer, String contentType, String body) {
        HttpURLConnection conn = null;
        try {
            URL u = URI.create(url).toURL();
            conn = (HttpURLConnection) u.openConnection();
            conn.setRequestMethod(method);
            conn.setConnectTimeout((int) options.connectTimeout.toMillis());
            conn.setReadTimeout((int) options.readTimeout.toMillis());
            conn.setRequestProperty("User-Agent", options.userAgent);
            conn.setRequestProperty("Accept", "application/json");
            if (bearer != null) conn.setRequestProperty("Authorization", "Bearer " + bearer);
            if (body != null) {
                conn.setDoOutput(true);
                if (contentType != null) conn.setRequestProperty("Content-Type", contentType);
                try (DataOutputStream os = new DataOutputStream(conn.getOutputStream())) {
                    os.write(body.getBytes(StandardCharsets.UTF_8));
                }
            }
            int status = conn.getResponseCode();
            String respBody = readAll(status >= 400 ? conn.getErrorStream() : conn.getInputStream());
            return new Response(status, respBody);
        } catch (IOException e) {
            throw new NetworkException(e.getMessage(), e);
        } finally {
            if (conn != null) conn.disconnect();
        }
    }

    private static String readAll(InputStream in) throws IOException {
        if (in == null) return "";
        try (BufferedReader r = new BufferedReader(new InputStreamReader(in, StandardCharsets.UTF_8))) {
            StringBuilder sb = new StringBuilder();
            String line;
            while ((line = r.readLine()) != null) {
                sb.append(line);
            }
            return sb.toString();
        }
    }
}

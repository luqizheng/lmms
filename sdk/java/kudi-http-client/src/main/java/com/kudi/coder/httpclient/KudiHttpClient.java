package com.kudi.coder.httpclient;

import com.kudi.coder.httpclient.Exceptions.AuthException;
import com.kudi.coder.httpclient.Exceptions.KudiHttpException;
import com.kudi.coder.httpclient.Exceptions.NetworkException;
import com.kudi.coder.httpclient.Exceptions.RateLimitException;
import com.kudi.coder.httpclient.Exceptions.ServerException;
import com.kudi.coder.httpclient.Exceptions.ValidationException;

public class KudiHttpClient {
    private final KudiHttpOptions options;
    private final HttpTransport transport;

    private KudiHttpClient(KudiHttpOptions options) {
        this.options = options;
        this.transport = new HttpTransport(options);
    }

    public static KudiHttpClient create() {
        return new KudiHttpClient(new KudiHttpOptions());
    }

    public static KudiHttpClient create(KudiHttpOptions options) {
        return new KudiHttpClient(options);
    }

    public KudiHttpOptions options() {
        return options;
    }

    public HttpTransport.Response get(String url, String bearer) {
        return sendWithRetry("GET", url, bearer, null, null);
    }

    public HttpTransport.Response post(String url, String jsonBody, String bearer) {
        return sendWithRetry("POST", url, bearer, "application/json", jsonBody);
    }

    public HttpTransport.Response put(String url, String jsonBody, String bearer) {
        return sendWithRetry("PUT", url, bearer, "application/json", jsonBody);
    }

    public HttpTransport.Response delete(String url, String bearer) {
        return sendWithRetry("DELETE", url, bearer, null, null);
    }

    private HttpTransport.Response sendWithRetry(String method, String url, String bearer,
                                                 String contentType, String body) {
        int attempts = Math.max(0, options.retryCount) + 1;
        NetworkException lastNetwork = null;
        HttpTransport.Response lastResp = null;
        for (int i = 0; i < attempts; i++) {
            try {
                HttpTransport.Response resp = transport.send(method, url, bearer, contentType, body);
                lastResp = resp;
                if (resp.statusCode >= 500 && i < attempts - 1) {
                    continue;
                }
                throwOnError(resp);
                return resp;
            } catch (NetworkException e) {
                lastNetwork = e;
                if (i >= attempts - 1) throw e;
            } catch (Exception e) {
                if (e instanceof KudiHttpException) throw (KudiHttpException) e;
                throw new NetworkException(e.getMessage(), e);
            }
        }
        if (lastResp != null) {
            throwOnError(lastResp);
            return lastResp;
        }
        throw lastNetwork != null ? lastNetwork : new NetworkException("request failed after retries");
    }

    private void throwOnError(HttpTransport.Response resp) {
        int code = resp.statusCode;
        if (code >= 200 && code < 300) return;
        if (code == 401 || code == 403) throw new AuthException("HTTP " + code, code, resp.body);
        if (code == 429) throw new RateLimitException("HTTP 429", resp.body);
        if (code == 400 || code == 422) throw new ValidationException("HTTP " + code, code, resp.body);
        if (code >= 500) throw new ServerException("HTTP " + code, code, resp.body);
        throw new KudiHttpException("HTTP " + code, code, resp.body);
    }
}

package io.digua.common;

import io.digua.auth.CredentialProvider;
import io.digua.auth.TokenCache;
import io.digua.exception.AuthException;
import io.digua.exception.NetworkException;
import io.digua.exception.RateLimitException;
import io.digua.exception.SdkException;
import io.digua.exception.ServerException;
import io.digua.exception.ValidationException;
import io.digua.http.HttpClientWrapper;
import io.digua.http.HttpRequest;
import io.digua.http.HttpResponse;
import io.digua.model.ApiError;

import java.util.concurrent.CompletableFuture;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;

public abstract class SdkClient implements AutoCloseable {
    protected final SdkConfig config;
    protected final HttpClientWrapper http;
    protected final TokenCache tokens;
    private final ExecutorService asyncExecutor;

    protected SdkClient(SdkConfig config, CredentialProvider provider) {
        this.config = config;
        this.http = new HttpClientWrapper(config);
        this.tokens = new TokenCache(provider);
        this.asyncExecutor = Executors.newCachedThreadPool(r -> {
            Thread t = new Thread(r, "digua-sdk-async");
            t.setDaemon(true);
            return t;
        });
    }

    protected HttpResponse executeWithAuth(HttpRequest request) {
        HttpResponse resp = doRequest(request);
        if (resp.getStatusCode() == 401) {
            tokens.invalidate();
            resp = doRequest(request);
        }
        ensureSuccess(resp);
        return resp;
    }

    private HttpResponse doRequest(HttpRequest request) {
        if (request.getHeaders().get("Authorization") == null) {
            request.bearer(tokens.getAccessToken());
        }
        return http.execute(request);
    }

    protected void ensureSuccess(HttpResponse resp) {
        int code = resp.getStatusCode();
        if (code >= 200 && code < 300) return;
        String details = resp.getBody();
        String message = "HTTP " + code;
        try {
            ApiError err = JsonUtil.fromJson(details, ApiError.class);
            if (err != null && err.getMessage() != null) {
                message = err.getMessage();
            }
        } catch (Exception ignored) {
        }
        if (code == 401 || code == 403) throw new AuthException(message, code, details);
        if (code == 429) throw new RateLimitException(message, details);
        if (code == 400 || code == 422) throw new ValidationException(message, code, details);
        if (code >= 500) throw new ServerException(message, code, details);
        throw new SdkException(message, code, details, null);
    }

    protected <T> T mapBody(HttpResponse resp, Class<T> type) {
        if (resp.getBody() == null || resp.getBody().isEmpty()) {
            return null;
        }
        return JsonUtil.fromJson(resp.getBody(), type);
    }

    protected <T> CompletableFuture<T> wrapAsync(java.util.concurrent.Callable<T> callable) {
        return CompletableFuture.supplyAsync(() -> {
            try {
                return callable.call();
            } catch (RuntimeException e) {
                throw e;
            } catch (Exception e) {
                throw new NetworkException(e.getMessage(), e);
            }
        }, asyncExecutor);
    }

    public SdkConfig getConfig() { return config; }

    @Override
    public void close() {
        asyncExecutor.shutdownNow();
    }
}

package com.kudi.coder.httpclient;

public class KudiWebClient {
    private final KudiHttpOptions options;
    private final HttpTransport transport;
    private final ServiceTokenCache cache;
    private final ClientCredentialsFetcher fetcher;
    private String clientId;
    private String secretKey;

    private KudiWebClient(KudiHttpOptions options) {
        this.options = options;
        this.transport = new HttpTransport(options);
        this.cache = new ServiceTokenCache(options.refreshSkew);
        this.fetcher = new ClientCredentialsFetcher(transport, () -> this.options.tokenEndpoint);
    }

    public static KudiWebClient create() {
        return new KudiWebClient(new KudiHttpOptions());
    }

    public static KudiWebClient create(KudiHttpOptions options) {
        return new KudiWebClient(options);
    }

    public KudiWebClient withCredential(String clientId, String secretKey) {
        this.clientId = clientId;
        this.secretKey = secretKey;
        return this;
    }

    public KudiWebClient tokenEndpoint(String url) {
        this.options.tokenEndpoint = url;
        return this;
    }

    public KudiHttpOptions options() {
        return options;
    }

    public int cacheSize() {
        return cache.size();
    }

    private String resolveBearer(String explicit) throws Exception {
        if (explicit != null) return explicit;
        if (clientId != null && secretKey != null) {
            return cache.getOrFetch(clientId, secretKey, () -> fetcher.fetch(clientId, secretKey));
        }
        return null;
    }

    public HttpTransport.Response post(String url, String jsonBody, String bearer) throws Exception {
        return transport.postJson(url, jsonBody, resolveBearer(bearer));
    }

    public HttpTransport.Response get(String url, String bearer) throws Exception {
        return transport.get(url, resolveBearer(bearer));
    }
}

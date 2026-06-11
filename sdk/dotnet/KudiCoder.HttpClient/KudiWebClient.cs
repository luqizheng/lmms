using KudiCoder.HttpClient.Exceptions;

namespace KudiCoder.HttpClient;

public class KudiWebClient : IAsyncDisposable
{
    private readonly HttpTransport _transport;
    private readonly KudiHttpOptions _options;
    private readonly ServiceTokenCache _cache;
    private readonly ClientCredentialsFetcher _fetcher;
    private (string ClientId, string SecretKey)? _credential;

    private KudiWebClient(HttpTransport transport, KudiHttpOptions options)
    {
        _transport = transport;
        _options = options;
        _cache = new ServiceTokenCache(options.RefreshSkew);
        _fetcher = new ClientCredentialsFetcher(transport, options.TokenEndpoint);
    }

    public static KudiWebClient Create(KudiHttpOptions? options = null)
    {
        var opts = options ?? new KudiHttpOptions();
        return new KudiWebClient(new HttpTransport(opts), opts);
    }

    /// <summary>
    /// Create a client backed by a caller-supplied HttpClient. Used by tests.
    /// </summary>
    public static KudiWebClient CreateWith(System.Net.Http.HttpClient httpClient, KudiHttpOptions? options = null)
    {
        var opts = options ?? new KudiHttpOptions();
        return new KudiWebClient(new HttpTransport(httpClient, opts), opts);
    }

    public KudiWebClient WithCredential(string clientId, string secretKey)
    {
        _credential = (clientId, secretKey);
        return this;
    }

    public KudiHttpOptions Options() => _options;
    public int CacheSize() => _cache.Size;

    private async Task<string?> ResolveBearerAsync(string? explicitBearer, CancellationToken ct)
    {
        if (explicitBearer != null) return explicitBearer;
        if (_credential is var (cid, sk))
            return await _cache.GetOrFetchAsync(cid, sk, () => _fetcher.FetchAsync(cid, sk, ct));
        return null;
    }

    public async Task<HttpResponse> PostAsync(string url, string jsonBody, string? bearer = null, CancellationToken ct = default)
    {
        var resolved = await ResolveBearerAsync(bearer, ct);
        return await _transport.PostJsonAsync(url, jsonBody, resolved, ct);
    }

    public async Task<HttpResponse> GetAsync(string url, string? bearer = null, CancellationToken ct = default)
    {
        var resolved = await ResolveBearerAsync(bearer, ct);
        return await _transport.GetAsync(url, resolved, ct);
    }

    public Task<HttpResponse> GetWithBearerAsync(string url, string bearer, CancellationToken ct = default)
        => _transport.GetAsync(url, bearer, ct);

    public ValueTask DisposeAsync()
    {
        GC.SuppressFinalize(this);
        return ValueTask.CompletedTask;
    }
}

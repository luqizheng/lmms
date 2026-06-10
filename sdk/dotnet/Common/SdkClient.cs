namespace KudiCoder.Sdk.Common;

public abstract class SdkClient : IAsyncDisposable
{
    protected SdkConfig Config { get; }
    protected HttpClientWrapper Http { get; }
    protected TokenCache Tokens { get; }

    protected SdkClient(SdkConfig config, ICredentialProvider provider)
    {
        Config = config;
        Http = new HttpClientWrapper(config);
        Tokens = new TokenCache(provider);
    }

    protected async Task<HttpResponse> ExecuteWithAuthAsync(HttpRequest request, CancellationToken ct = default)
    {
        var resp = await DoRequestAsync(request, ct);
        if (resp.StatusCode == 401)
        {
            await Tokens.InvalidateAsync();
            resp = await DoRequestAsync(request, ct);
        }
        EnsureSuccess(resp);
        return resp;
    }

    private async Task<HttpResponse> DoRequestAsync(HttpRequest request, CancellationToken ct)
    {
        if (!request.Headers.ContainsKey("Authorization"))
        {
            var token = await Tokens.GetAccessTokenAsync(ct);
            request.WithBearer(token);
        }
        return await Http.ExecuteAsync(request, ct);
    }

    protected void EnsureSuccess(HttpResponse resp)
    {
        if (resp.StatusCode >= 200 && resp.StatusCode < 300) return;
        var details = resp.Body;
        var message = $"HTTP {resp.StatusCode}";
        try
        {
            var err = JsonUtil.FromJson<ApiError>(details);
            if (err != null && !string.IsNullOrEmpty(err.Message)) message = err.Message;
        }
        catch { }
        switch (resp.StatusCode)
        {
            case 401:
            case 403: throw new AuthException(message, resp.StatusCode, details);
            case 429: throw new RateLimitException(message, details);
            case 400:
            case 422: throw new ValidationException(message, resp.StatusCode, details);
        }
        if (resp.StatusCode >= 500) throw new ServerException(message, resp.StatusCode, details);
        throw new SdkException(message, resp.StatusCode, details, null);
    }

    protected T? MapBody<T>(HttpResponse resp) where T : class
    {
        if (string.IsNullOrEmpty(resp.Body)) return null;
        return JsonUtil.FromJson<T>(resp.Body);
    }

    protected Task<T> WrapAsync<T>(Func<Task<T>> func) => Task.Run(func);

    public SdkConfig GetConfig() => Config;

    public ValueTask DisposeAsync()
    {
        GC.SuppressFinalize(this);
        return ValueTask.CompletedTask;
    }
}

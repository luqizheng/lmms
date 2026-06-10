using System.Text.Json.Serialization;
using KudiCoder.Sdk.Common;

namespace KudiCoder.TokenServer.ClientSdk;

public class ApiKeyInfo
{
    [JsonPropertyName("api_key")] public string? ApiKey { get; set; }
    [JsonPropertyName("secret")] public string? Secret { get; set; }
    [JsonPropertyName("created_at")] public string? CreatedAt { get; set; }
}

public class TokenServerClientClient : SdkClient
{
    private readonly string _basePath;

    private TokenServerClientClient(SdkConfig config, ICredentialProvider provider, string basePath)
        : base(config, provider) { _basePath = basePath; }

    public static TokenServerClientClient Create(string baseUrl, string apiKey, string secret, string code)
        => Create(new SdkConfig(baseUrl), apiKey, secret, code);

    public static TokenServerClientClient Create(SdkConfig config, string apiKey, string secret, string code)
        => new(config, new ClientCredentialsProvider(config, apiKey, secret, code), "/api");

    private string Url(string suffix) => Config.BaseUrl + _basePath + suffix;

    public async Task<TokenInfo> RefreshAsync(string refreshToken, string clientId, string clientSecret, CancellationToken ct = default)
    {
        var body = await new FormUrlEncodedContent(new Dictionary<string, string>
        {
            ["grant_type"] = "refresh_token",
            ["refresh_token"] = refreshToken,
            ["client_id"] = clientId,
            ["client_secret"] = clientSecret
        }).ReadAsStringAsync(ct);
        var r = await ExecuteWithAuthAsync(HttpRequest.Post(Url("/tokens/refresh")).WithBody(body, "application/x-www-form-urlencoded"), ct);
        return MapBody<TokenInfo>(r) ?? new();
    }

    public async Task RevokeAsync(string token, string clientId, string clientSecret, CancellationToken ct = default)
    {
        var body = await new FormUrlEncodedContent(new Dictionary<string, string>
        {
            ["token"] = token,
            ["client_id"] = clientId,
            ["client_secret"] = clientSecret
        }).ReadAsStringAsync(ct);
        var r = await ExecuteWithAuthAsync(HttpRequest.Post(Url("/tokens/revoke")).WithBody(body, "application/x-www-form-urlencoded"), ct);
        EnsureSuccess(r);
    }

    public async Task<TokenInfo> IssueResourceAsync(string userId, string resource, string scope, string clientId, string clientSecret, CancellationToken ct = default)
    {
        var body = await new FormUrlEncodedContent(new Dictionary<string, string>
        {
            ["client_id"] = clientId,
            ["client_secret"] = clientSecret,
            ["user_id"] = userId,
            ["resource"] = resource,
            ["scope"] = scope
        }).ReadAsStringAsync(ct);
        var r = await ExecuteWithAuthAsync(HttpRequest.Post(Url("/tokens/resource")).WithBody(body, "application/x-www-form-urlencoded"), ct);
        return MapBody<TokenInfo>(r) ?? new();
    }

    public async Task<ApiKeyInfo?> CreateApiKeyAsync(string name, string clientId, string clientSecret, CancellationToken ct = default)
    {
        var body = await new FormUrlEncodedContent(new Dictionary<string, string>
        {
            ["client_id"] = clientId,
            ["client_secret"] = clientSecret,
            ["name"] = name
        }).ReadAsStringAsync(ct);
        var r = await ExecuteWithAuthAsync(HttpRequest.Post(Url("/api-keys")).WithBody(body, "application/x-www-form-urlencoded"), ct);
        return MapBody<ApiKeyInfo>(r);
    }

    public async Task<string> GetJwksAsync(CancellationToken ct = default)
    {
        var r = await ExecuteWithAuthAsync(HttpRequest.Get(Url("/tokens/.well-known/jwks.json")), ct);
        return r.Body;
    }
}

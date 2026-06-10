using KudiCoder.Sdk.Common;

namespace KudiCoder.TokenServer.UserSdk;

public class TokenServerUserClient : SdkClient
{
    private readonly string _basePath;

    private TokenServerUserClient(SdkConfig config, ICredentialProvider provider, string basePath)
        : base(config, provider) { _basePath = basePath; }

    public static TokenServerUserClient Login(string baseUrl, string username, string password)
        => Login(new SdkConfig(baseUrl), username, password);

    public static TokenServerUserClient Login(SdkConfig config, string username, string password)
        => new(config, new PasswordCredentialProvider(config, username, password, "/api/tokens/access"), "/api");

    private string Url(string suffix) => Config.BaseUrl + _basePath + suffix;

    public async Task<TokenInfo> LoginAsync(string code, string clientId, string clientSecret, CancellationToken ct = default)
    {
        var body = await new FormUrlEncodedContent(new Dictionary<string, string>
        {
            ["grant_type"] = "authorization_code",
            ["code"] = code,
            ["client_id"] = clientId,
            ["client_secret"] = clientSecret
        }).ReadAsStringAsync(ct);
        var r = await ExecuteWithAuthAsync(HttpRequest.Post(Url("/tokens/access")).WithBody(body, "application/x-www-form-urlencoded"), ct);
        return MapBody<TokenInfo>(r) ?? new();
    }

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

    public async Task<string> GetJwksAsync(CancellationToken ct = default)
    {
        var r = await ExecuteWithAuthAsync(HttpRequest.Get(Url("/tokens/.well-known/jwks.json")), ct);
        return r.Body;
    }
}

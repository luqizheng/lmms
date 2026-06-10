using System.Collections.Generic;
using System.Net;

namespace KudiCoder.Sdk.Common;

public interface ICredentialProvider
{
    Task<TokenInfo> FetchTokenAsync(CancellationToken ct = default);
    void Invalidate();
}

public class PasswordCredentialProvider : ICredentialProvider
{
    private readonly SdkConfig _config;
    private readonly HttpClientWrapper _http;
    private readonly string _username;
    private readonly string _password;
    private readonly string _loginPath;

    public PasswordCredentialProvider(SdkConfig config, string username, string password,
        string loginPath = "/api/v1/auth/login")
    {
        _config = config;
        _http = new HttpClientWrapper(config);
        _username = username;
        _password = password;
        _loginPath = loginPath;
    }

    public async Task<TokenInfo> FetchTokenAsync(CancellationToken ct = default)
    {
        var form = new Dictionary<string, string>
        {
            ["username"] = _username,
            ["password"] = _password
        };
        var body = await new FormUrlEncodedContent(form).ReadAsStringAsync(ct);
        var req = HttpRequest.Post(_config.BaseUrl + _loginPath)
            .WithBody(body, "application/x-www-form-urlencoded");
        var resp = await _http.ExecuteAsync(req, ct);
        if (resp.StatusCode == 401 || resp.StatusCode == 403)
            throw new AuthException("login failed", resp.StatusCode, resp.Body);
        if (resp.StatusCode < 200 || resp.StatusCode >= 300)
            throw new SdkException($"login failed: HTTP {resp.StatusCode}", resp.StatusCode, resp.Body, null);
        var t = JsonUtil.FromJson<TokenInfo>(resp.Body)
            ?? throw new SdkException("login response empty");
        if (string.IsNullOrEmpty(t.AccessToken))
            throw new SdkException("login response missing access_token");
        return t;
    }

    public void Invalidate() { }
}

public class ClientCredentialsProvider : ICredentialProvider
{
    private readonly SdkConfig _config;
    private readonly HttpClientWrapper _http;
    private readonly string _clientId;
    private readonly string _clientSecret;
    private readonly string _code;
    private readonly string _tokenPath;

    public ClientCredentialsProvider(SdkConfig config, string clientId, string clientSecret, string code,
        string tokenPath = "/api/tokens/access")
    {
        _config = config;
        _http = new HttpClientWrapper(config);
        _clientId = clientId;
        _clientSecret = clientSecret;
        _code = code;
        _tokenPath = tokenPath;
    }

    public async Task<TokenInfo> FetchTokenAsync(CancellationToken ct = default)
    {
        var form = new Dictionary<string, string>
        {
            ["grant_type"] = "authorization_code",
            ["code"] = _code,
            ["client_id"] = _clientId,
            ["client_secret"] = _clientSecret
        };
        var body = await new FormUrlEncodedContent(form).ReadAsStringAsync(ct);
        var req = HttpRequest.Post(_config.BaseUrl + _tokenPath)
            .WithBody(body, "application/x-www-form-urlencoded");
        var resp = await _http.ExecuteAsync(req, ct);
        if (resp.StatusCode == 401 || resp.StatusCode == 403)
            throw new AuthException("client_credentials failed", resp.StatusCode, resp.Body);
        if (resp.StatusCode < 200 || resp.StatusCode >= 300)
            throw new SdkException($"client_credentials failed: HTTP {resp.StatusCode}", resp.StatusCode, resp.Body, null);
        var t = JsonUtil.FromJson<TokenInfo>(resp.Body)
            ?? throw new SdkException("token response empty");
        if (string.IsNullOrEmpty(t.AccessToken))
            throw new SdkException("token response missing access_token");
        return t;
    }

    public void Invalidate() { }
}

public class StaticTokenProvider : ICredentialProvider
{
    private string _accessToken;
    private readonly long _expiresInSec;

    public StaticTokenProvider(string accessToken, long expiresInSec = 3600)
    {
        _accessToken = accessToken;
        _expiresInSec = expiresInSec;
    }

    public Task<TokenInfo> FetchTokenAsync(CancellationToken ct = default)
    {
        return Task.FromResult(new TokenInfo
        {
            AccessToken = _accessToken,
            ExpiresIn = _expiresInSec,
            TokenType = "Bearer"
        });
    }

    public void Invalidate() { }

    public void SetAccessToken(string token) { _accessToken = token; }
}

public class TokenCache
{
    private static readonly TimeSpan RefreshSkew = TimeSpan.FromSeconds(60);
    private readonly ICredentialProvider _provider;
    private readonly SemaphoreSlim _lock = new(1, 1);
    private TokenInfo? _cached;
    private DateTime _expiresAtUtc = DateTime.MinValue;

    public TokenCache(ICredentialProvider provider) { _provider = provider; }

    public async Task<string> GetAccessTokenAsync(CancellationToken ct = default)
    {
        if (_cached != null && DateTime.UtcNow < _expiresAtUtc - RefreshSkew)
            return _cached.AccessToken!;
        await _lock.WaitAsync(ct);
        try
        {
            if (_cached == null || DateTime.UtcNow >= _expiresAtUtc - RefreshSkew)
            {
                _cached = await _provider.FetchTokenAsync(ct);
                _expiresAtUtc = DateTime.UtcNow.AddSeconds(_cached.ExpiresIn);
                if (string.IsNullOrEmpty(_cached.AccessToken))
                    throw new SdkException("credential provider returned empty token");
            }
            return _cached.AccessToken!;
        }
        finally { _lock.Release(); }
    }

    public async Task InvalidateAsync()
    {
        await _lock.WaitAsync();
        try
        {
            _cached = null;
            _expiresAtUtc = DateTime.MinValue;
            _provider.Invalidate();
        }
        finally { _lock.Release(); }
    }
}

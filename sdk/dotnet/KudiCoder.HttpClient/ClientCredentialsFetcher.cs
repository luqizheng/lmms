using System.Text.Json;
using KudiCoder.HttpClient.Exceptions;
using KudiCoder.HttpClient.Models;

namespace KudiCoder.HttpClient;

public class ClientCredentialsFetcher
{
    private static readonly JsonSerializerOptions JsonOptions = new()
    {
        PropertyNameCaseInsensitive = true,
    };

    private readonly HttpTransport _transport;
    private readonly string _tokenEndpoint;

    public ClientCredentialsFetcher(HttpTransport transport, string tokenEndpoint)
    {
        _transport = transport;
        _tokenEndpoint = tokenEndpoint;
    }

    public async Task<TokenInfo> FetchAsync(string clientId, string clientSecret, CancellationToken ct = default)
    {
        var form = new Dictionary<string, string>
        {
            ["grant_type"] = "client_credentials",
            ["client_id"] = clientId,
            ["client_secret"] = clientSecret,
        };
        var resp = await _transport.PostFormAsync(_tokenEndpoint, form, ct);
        if (resp.StatusCode is < 200 or >= 300)
            throw new AuthException($"client_credentials failed: HTTP {resp.StatusCode}", resp.StatusCode, resp.Body);
        TokenInfo? info;
        try
        {
            info = JsonSerializer.Deserialize<TokenInfo>(resp.Body, JsonOptions);
        }
        catch (JsonException e)
        {
            throw new NetworkException("parse token response: " + e.Message, e);
        }
        if (info == null || string.IsNullOrEmpty(info.AccessToken))
            throw new AuthException("token response missing access_token", resp.StatusCode, resp.Body);
        return info;
    }
}

using System.Net.Http.Headers;
using System.Text;
using KudiCoder.HttpClient.Exceptions;

namespace KudiCoder.HttpClient;

public class HttpResponse
{
    public int StatusCode { get; }
    public string Body { get; }
    public HttpResponse(int statusCode, string body)
    {
        StatusCode = statusCode;
        Body = body;
    }
}

public class HttpTransport
{
    private readonly KudiHttpOptions _options;
    private readonly System.Net.Http.HttpClient? _injectedClient;

    public HttpTransport(KudiHttpOptions options)
    {
        _options = options;
        _injectedClient = null;
    }

    /// <summary>
    /// Construct a transport backed by a caller-supplied HttpClient.
    /// Used by tests to inject a stub HttpMessageHandler.
    /// </summary>
    public HttpTransport(System.Net.Http.HttpClient client, KudiHttpOptions options)
    {
        _injectedClient = client;
        _options = options;
    }

    public Task<HttpResponse> PostFormAsync(string url, IDictionary<string, string> form, CancellationToken ct = default)
        => SendAsync(HttpMethod.Post, url, null, new FormUrlEncodedContent(form), ct);

    public Task<HttpResponse> PostJsonAsync(string url, string json, string? bearer, CancellationToken ct = default)
        => SendAsync(HttpMethod.Post, url, bearer, new StringContent(json, Encoding.UTF8, "application/json"), ct);

    public Task<HttpResponse> GetAsync(string url, string? bearer, CancellationToken ct = default)
        => SendAsync(HttpMethod.Get, url, bearer, null, ct);

    private async Task<HttpResponse> SendAsync(HttpMethod method, string url, string? bearer, HttpContent? content, CancellationToken ct)
    {
        using var req = new System.Net.Http.HttpRequestMessage(method, url);
        if (content != null) req.Content = content;
        if (bearer != null) req.Headers.Authorization = new AuthenticationHeaderValue("Bearer", bearer);

        if (_injectedClient != null)
        {
            try
            {
                using var resp = await _injectedClient.SendAsync(req, ct);
                var body = await resp.Content.ReadAsStringAsync(ct);
                return new HttpResponse((int)resp.StatusCode, body);
            }
            catch (HttpRequestException e) { throw new NetworkException(e.Message, e); }
            catch (TaskCanceledException e) { throw new NetworkException("request timeout", e); }
        }

        using var http = new System.Net.Http.HttpClient
        {
            Timeout = TimeSpan.FromMilliseconds(_options.ReadTimeoutMs + _options.ConnectTimeoutMs),
        };
        req.Headers.UserAgent.ParseAdd(_options.UserAgent);
        try
        {
            using var resp = await http.SendAsync(req, ct);
            var body = await resp.Content.ReadAsStringAsync(ct);
            return new HttpResponse((int)resp.StatusCode, body);
        }
        catch (HttpRequestException e) { throw new NetworkException(e.Message, e); }
        catch (TaskCanceledException e) { throw new NetworkException("request timeout", e); }
    }
}

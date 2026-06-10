using System.Net.Http.Headers;
using System.Text;

namespace KudiCoder.Sdk.Common;

public class HttpRequest
{
    public string Method { get; set; } = "GET";
    public string Url { get; set; } = "";
    public Dictionary<string, string> Headers { get; } = new();
    public string? Body { get; set; }

    public HttpRequest() { }

    public HttpRequest(string method, string url)
    {
        Method = method;
        Url = url;
    }

    public static HttpRequest Get(string url) => new("GET", url);
    public static HttpRequest Post(string url) => new("POST", url);
    public static HttpRequest Put(string url) => new("PUT", url);
    public static HttpRequest Delete(string url) => new("DELETE", url);
    public static HttpRequest Patch(string url) => new("PATCH", url);

    public HttpRequest WithHeader(string name, string value)
    {
        Headers[name] = value;
        return this;
    }

    public HttpRequest WithBearer(string token) => WithHeader("Authorization", "Bearer " + token);

    public HttpRequest WithJsonBody(object value)
    {
        Body = JsonUtil.ToJson(value);
        WithHeader("Content-Type", "application/json");
        return this;
    }

    public HttpRequest WithBody(string body, string contentType)
    {
        Body = body;
        WithHeader("Content-Type", contentType);
        return this;
    }
}

public class HttpResponse
{
    public int StatusCode { get; }
    public string Body { get; }
    public HttpResponse(int statusCode, string body) { StatusCode = statusCode; Body = body; }
}

public class HttpClientWrapper
{
    private readonly SdkConfig _config;

    public HttpClientWrapper(SdkConfig config)
    {
        _config = config;
    }

    public async Task<HttpResponse> ExecuteAsync(HttpRequest request, CancellationToken ct = default)
    {
        using var http = new HttpClient
        {
            Timeout = TimeSpan.FromMilliseconds(_config.ReadTimeoutMs + _config.ConnectTimeoutMs)
        };
        using var msg = new HttpRequestMessage(new HttpMethod(request.Method), request.Url);
        if (request.Body != null)
        {
            msg.Content = new StringContent(request.Body, Encoding.UTF8);
            if (request.Headers.TryGetValue("Content-Type", out var ct2))
            {
                msg.Content.Headers.ContentType = MediaTypeHeaderValue.Parse(ct2);
            }
        }
        foreach (var kv in request.Headers)
        {
            if (kv.Key.Equals("Content-Type", StringComparison.OrdinalIgnoreCase)) continue;
            msg.Headers.TryAddWithoutValidation(kv.Key, kv.Value);
        }
        msg.Headers.UserAgent.ParseAdd(_config.UserAgent);
        msg.Headers.Accept.ParseAdd("application/json");
        try
        {
            using var resp = await http.SendAsync(msg, ct);
            var body = await resp.Content.ReadAsStringAsync(ct);
            return new HttpResponse((int)resp.StatusCode, body);
        }
        catch (HttpRequestException e) { throw new NetworkException(e.Message, e); }
        catch (TaskCanceledException e) { throw new NetworkException("request timeout", e); }
    }
}

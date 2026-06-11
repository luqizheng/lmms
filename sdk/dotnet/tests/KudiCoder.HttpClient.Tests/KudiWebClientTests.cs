using System.Net;
using Xunit;

[Collection(nameof(ServiceTokenCacheCollection))]
public class KudiWebClientTests
{
    public KudiWebClientTests() { }

    private sealed class StubHandler : HttpMessageHandler
    {
        public int TokenCalls;
        public int DownstreamCalls;
        public string? LastAuthHeader;
        public string TokenBody = "{\"access_token\":\"svc-token\",\"token_type\":\"Bearer\",\"expires_in\":3600}";
        public string DownstreamBody = "{}";
        public HttpStatusCode DownstreamStatus = HttpStatusCode.OK;

        protected override async Task<HttpResponseMessage> SendAsync(HttpRequestMessage req, CancellationToken ct)
        {
            var path = req.RequestUri!.AbsolutePath;
            if (path.EndsWith("/api/tokens/access", StringComparison.Ordinal))
            {
                TokenCalls++;
                return new HttpResponseMessage(HttpStatusCode.OK) { Content = new StringContent(TokenBody) };
            }
            DownstreamCalls++;
            LastAuthHeader = req.Headers.Authorization?.Parameter;
            return new HttpResponseMessage(DownstreamStatus) { Content = new StringContent(DownstreamBody) };
        }
    }

    private static (string cid, string sk) UniqueCred()
        => ("cid-" + Guid.NewGuid().ToString("N"), "sk-" + Guid.NewGuid().ToString("N"));

    [Fact]
    public async Task FetchesTokenOnceAndUsesForSubsequentCalls()
    {
        var (cid, sk) = UniqueCred();
        var handler = new StubHandler();
        var http = new HttpClient(handler);
        var client = KudiCoder.HttpClient.KudiWebClient
            .CreateWith(http, new KudiCoder.HttpClient.KudiHttpOptions { TokenEndpoint = "http://t/api/tokens/access" })
            .WithCredential(cid, sk);

        await client.GetAsync("http://u/api/x");
        await client.GetAsync("http://u/api/y");
        await client.PostAsync("http://u/api/z", "{}");

        Assert.Equal(1, handler.TokenCalls);
        Assert.Equal(3, handler.DownstreamCalls);
        Assert.Equal("svc-token", handler.LastAuthHeader);
    }

    [Fact]
    public async Task UserBearerPassthroughSkipsTokenFetch()
    {
        var (cid, sk) = UniqueCred();
        var handler = new StubHandler();
        var http = new HttpClient(handler);
        var client = KudiCoder.HttpClient.KudiWebClient
            .CreateWith(http, new KudiCoder.HttpClient.KudiHttpOptions { TokenEndpoint = "http://t/api/tokens/access" })
            .WithCredential(cid, sk);

        await client.GetWithBearerAsync("http://u/api/me", "user.jwt");
        Assert.Equal(0, handler.TokenCalls);
        Assert.Equal("user.jwt", handler.LastAuthHeader);
    }
}

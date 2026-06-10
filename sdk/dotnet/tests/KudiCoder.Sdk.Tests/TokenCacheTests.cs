using KudiCoder.Sdk.Common;
using Xunit;

namespace KudiCoder.Sdk.Tests;

public class TokenCacheTests
{
    [Fact]
    public async Task CachesUntilCloseToExpiry()
    {
        int calls = 0;
        var provider = new CountingProvider(() =>
        {
            calls++;
            return new TokenInfo { AccessToken = "a-1", ExpiresIn = 120, TokenType = "Bearer" };
        });
        var cache = new TokenCache(provider);
        Assert.Equal("a-1", await cache.GetAccessTokenAsync());
        Assert.Equal("a-1", await cache.GetAccessTokenAsync());
        Assert.Equal(1, calls);
    }

    [Fact]
    public async Task InvalidateForcesRefresh()
    {
        int n = 0;
        var provider = new CountingProvider(() =>
        {
            n++;
            return new TokenInfo { AccessToken = "a-" + n, ExpiresIn = 120, TokenType = "Bearer" };
        });
        var cache = new TokenCache(provider);
        var a = await cache.GetAccessTokenAsync();
        await cache.InvalidateAsync();
        var b = await cache.GetAccessTokenAsync();
        Assert.Equal("a-1", a);
        Assert.Equal("a-2", b);
        Assert.NotEqual(a, b);
    }

    private class CountingProvider : ICredentialProvider
    {
        private readonly Func<TokenInfo> _produce;
        public CountingProvider(Func<TokenInfo> produce) { _produce = produce; }
        public Task<TokenInfo> FetchTokenAsync(CancellationToken ct = default) => Task.FromResult(_produce());
        public void Invalidate() { }
    }
}

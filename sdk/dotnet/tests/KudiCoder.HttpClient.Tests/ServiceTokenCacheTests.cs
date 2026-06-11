using KudiCoder.HttpClient;
using KudiCoder.HttpClient.Models;
using Xunit;

[Collection(nameof(ServiceTokenCacheCollection))]
public class ServiceTokenCacheTests
{
    public ServiceTokenCacheTests() { }

    private static readonly System.Collections.Generic.List<string> RegisteredKeys = new();

    private static (string cid, string sk) UniqueKey()
    {
        var cid = "cid-" + Guid.NewGuid().ToString("N");
        var sk = "sk-" + Guid.NewGuid().ToString("N");
        return (cid, sk);
    }

    private static TokenInfo Info() => new() { AccessToken = "tok", ExpiresIn = 3600 };

    [Fact]
    public async Task CachesTokenPerCredentialPair()
    {
        var (cid, sk) = UniqueKey();
        var cache = new ServiceTokenCache(TimeSpan.FromSeconds(60));
        int calls = 0;
        async Task<TokenInfo> Fetch()
        {
            calls++;
            return Info();
        }
        var t1 = await cache.GetOrFetchAsync(cid, sk, Fetch);
        var t2 = await cache.GetOrFetchAsync(cid, sk, Fetch);
        Assert.Equal("tok", t1);
        Assert.Equal("tok", t2);
        Assert.Equal(1, calls);
    }

    [Fact]
    public async Task SeparateCredentialsHaveSeparateCacheEntries()
    {
        var (cid1, sk1) = UniqueKey();
        var (cid2, sk2) = UniqueKey();
        var cache = new ServiceTokenCache(TimeSpan.FromSeconds(60));
        int calls = 0;
        Task<TokenInfo> Fetch()
        {
            calls++;
            return Task.FromResult(Info());
        }
        await cache.GetOrFetchAsync(cid1, sk1, Fetch);
        await cache.GetOrFetchAsync(cid2, sk2, Fetch);
        Assert.Equal(2, calls);
        Assert.True(cache.Size >= 2);
    }

    [Fact]
    public async Task InvalidateRefetches()
    {
        var (cid, sk) = UniqueKey();
        var cache = new ServiceTokenCache(TimeSpan.FromSeconds(60));
        int calls = 0;
        Task<TokenInfo> Fetch()
        {
            calls++;
            return Task.FromResult(Info());
        }
        await cache.GetOrFetchAsync(cid, sk, Fetch);
        await cache.GetOrFetchAsync(cid, sk, Fetch);
        Assert.Equal(1, calls);
        cache.Invalidate(cid, sk);
        await cache.GetOrFetchAsync(cid, sk, Fetch);
        Assert.Equal(2, calls);
    }
}

[CollectionDefinition(nameof(ServiceTokenCacheCollection), DisableParallelization = true)]
public class ServiceTokenCacheCollection { }

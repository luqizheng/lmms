using System.Collections.Concurrent;
using System.Security.Cryptography;
using System.Text;
using KudiCoder.HttpClient.Models;

namespace KudiCoder.HttpClient;

public class ServiceTokenCache
{
    private static readonly ConcurrentDictionary<string, CachedToken> Store = new();
    private readonly TimeSpan _refreshSkew;

    public ServiceTokenCache(TimeSpan refreshSkew)
    {
        _refreshSkew = refreshSkew;
    }

    public int Size => Store.Count;

    public async Task<string> GetOrFetchAsync(string clientId, string secretKey, Func<Task<TokenInfo>> fetcher)
    {
        var key = Digest(clientId, secretKey);
        if (Store.TryGetValue(key, out var cached) && cached.Matches(clientId, secretKey) && cached.ExpiresAt - DateTime.UtcNow > _refreshSkew)
            return cached.Info.AccessToken;

        var info = await fetcher();
        var entry = new CachedToken(clientId, secretKey, info, DateTime.UtcNow.AddSeconds(info.ExpiresIn));
        Store[key] = entry;
        return info.AccessToken;
    }

    public void Invalidate(string clientId, string secretKey)
        => Store.TryRemove(Digest(clientId, secretKey), out _);

    private static string Digest(string a, string b)
    {
        using var sha = SHA256.Create();
        var bytes = Encoding.UTF8.GetBytes(a + "|" + b);
        return Convert.ToHexString(sha.ComputeHash(bytes));
    }

    private sealed class CachedToken
    {
        public string ClientId { get; }
        public string SecretKey { get; }
        public TokenInfo Info { get; }
        public DateTime ExpiresAt { get; }
        public CachedToken(string cid, string sk, TokenInfo info, DateTime exp)
        {
            ClientId = cid;
            SecretKey = sk;
            Info = info;
            ExpiresAt = exp;
        }
        public bool Matches(string cid, string sk) => ClientId == cid && SecretKey == sk;
    }
}

# KudiCoder.HttpClient

Generic HTTP client for inter-service calls in coder-micro-services. Two usage modes:

- **Service-to-service** — call `KudiWebClient.WithCredential(clientId, secretKey)` once, then any `GetAsync`/`PostAsync` automatically acquires and caches a `client_credentials` access token from `token-service`.
- **User Bearer passthrough** — call `GetWithBearerAsync(url, userJwt)` to forward an existing user JWT verbatim to a downstream service.

Token cache is keyed by `SHA256(clientId | secretKey)` and shared across instances. Default `expires_in - 60s` pre-refresh.

## Install

```xml
<ProjectReference Include="..\KudiCoder.HttpClient\KudiCoder.HttpClient.csproj" />
```

Targets `net6.0;net9.0;net10.0`.

## Usage

```csharp
using KudiCoder.HttpClient;

// Create once, reuse across requests
var client = KudiCoder.HttpClient.KudiWebClient.Create()
    .WithCredential("contract-svc", "secret-111");

// service-to-service: token auto-fetched, then cached
var resp = await client.GetAsync("http://user-svc/api/users/123");
Console.WriteLine($"{resp.StatusCode} {resp.Body}");

// pass a user token through
var userJwt = ctx.Request.Headers["Authorization"].ToString().Replace("Bearer ", "");
var me = await client.GetWithBearerAsync("http://user-svc/api/users/me", userJwt);

// POST JSON
await client.PostAsync("http://user-svc/api/users", "{\"name\":\"x\"}");
```

## Errors

Throws `KudiCoder.HttpClient.Exceptions.*`:

- `AuthException` — HTTP 401/403
- `ValidationException` — HTTP 400/422
- `RateLimitException` — HTTP 429
- `ServerException` — HTTP 5xx
- `NetworkException` — connection/timeout

## Options

`KudiHttpOptions`:

| Field | Default |
|---|---|
| `TokenEndpoint` | `http://token-svc/api/tokens/access` |
| `ConnectTimeoutMs` | 5000 |
| `ReadTimeoutMs` | 15000 |
| `RetryCount` | 1 (5xx/network only) |
| `RefreshSkew` | 60s |
| `UserAgent` | `kudi-http-client/0.1.0` |

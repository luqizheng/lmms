# kudi-http-client (Java)

Generic HTTP client for inter-service calls. Two modes:

- **Service-to-service** — `KudiWebClient.withCredential(clientId, secretKey)` once; subsequent `get`/`post` automatically fetch and cache a `client_credentials` token.
- **User Bearer passthrough** — pass an existing user JWT to forward to a downstream service.

Token cache is keyed by `SHA256(clientId | secretKey)` and shared across instances. Default `expires_in - 60s` pre-refresh.

## Install

```xml
<dependency>
  <groupId>com.kudi.coder</groupId>
  <artifactId>kudi-http-client</artifactId>
  <version>0.1.0</version>
</dependency>
```

## Usage

```java
import com.kudi.coder.httpclient.KudiWebClient;

KudiWebClient client = KudiWebClient.create()
    .withCredential("contract-svc", "secret-111");

// service-to-service: token auto-fetched + cached
HttpTransport.Response resp = client.get("http://user-svc/api/users/123", null);
System.out.println(resp.statusCode + " " + resp.body);

// user Bearer passthrough
client.get("http://user-svc/api/users/me", "user.jwt");

// POST JSON
client.post("http://user-svc/api/users", "{\"name\":\"x\"}", null);
```

## Errors

`com.kudi.coder.httpclient.Exceptions.*`:

- `AuthException` — HTTP 401/403
- `ValidationException` — HTTP 400/422
- `RateLimitException` — HTTP 429
- `ServerException` — HTTP 5xx
- `NetworkException` — connection/timeout

## Options

`KudiHttpOptions`:

| Field | Default |
|---|---|
| `tokenEndpoint` | `http://token-svc/api/tokens/access` |
| `connectTimeout` | 5s |
| `readTimeout` | 15s |
| `retryCount` | 1 (5xx/network only) |
| `refreshSkew` | 60s |
| `userAgent` | `kudi-http-client/0.1.0` |

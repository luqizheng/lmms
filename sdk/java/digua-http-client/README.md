# digua-http-client (Java)

轻量 HTTP 客户端，专为本平台的 **JWT 透传**模型设计。

> 本平台的鉴权模型是：上游收到的 `Authorization: Bearer <jwt>` 直接转发给下游，下游服务从 JWT payload 解析身份（不验签名，见 `libs/jwt-ctx`）。此模块不做 token 获取 / 缓存，只负责把已有的 JWT 透传到下游 HTTP 调用。
>
> 如果需要服务首次获取 token，请用：
>
> - 终端用户场景：`digua-user-user-sdk`（username/password 登录 → JWT）
> - OAuth Client 场景：`digua-token-client-sdk`（authorization_code → JWT）

## 安装

```xml
<dependency>
  <groupId>io.digua</groupId>
  <artifactId>digua-http-client</artifactId>
  <version>0.2.0</version>
</dependency>
```

## 用法

```java
import io.digua.httpclient.KudiHttpClient;
import io.digua.httpclient.HttpTransport;

KudiHttpClient client = KudiHttpClient.create();

// 透传上游收到的用户 JWT 给下游
HttpTransport.Response resp = client.get(
    "http://localhost:3000/v1/iam/users/me", userJwt);
System.out.println(resp.statusCode + " " + resp.body);

// POST JSON
client.post("http://localhost:3000/v1/iam/users",
    "{\"username\":\"alice\"}", userJwt);

// PUT / DELETE
client.put("http://localhost:3000/v1/iam/users/u-1", "{...}", userJwt);
client.delete("http://localhost:3000/v1/iam/users/u-1", userJwt);

// 也支持匿名调用（健康检查、登录等公开端点）
client.get("http://localhost:3000/v1/iam/health", null);
```

## 错误映射

非 2xx 响应会按状态码抛出 `io.digua.httpclient.Exceptions.*`：

| 状态码 | 异常 |
|--------|------|
| 401 / 403 | `AuthException` |
| 400 / 422 | `ValidationException` |
| 429 | `RateLimitException` |
| 5xx | `ServerException` |
| 其它非 2xx | `KudiHttpException`（基类） |
| 网络 / 超时 | `NetworkException` |

所有异常都携带 `statusCode` 与原始 `details`（响应体），方便排障。

## 重试

默认对 5xx 与网络错误重试 1 次。通过 `KudiHttpOptions.retryCount` 调整：

```java
KudiHttpOptions opts = new KudiHttpOptions();
opts.retryCount = 3;
KudiHttpClient client = KudiHttpClient.create(opts);
```

非 5xx 业务错误（4xx）不重试，立即抛对应异常。

## 配置

`KudiHttpOptions`：

| 字段 | 默认值 | 说明 |
|------|--------|------|
| `connectTimeout` | 5s | 连接超时 |
| `readTimeout` | 15s | 读超时 |
| `retryCount` | 1 | 5xx / 网络错误重试次数 |
| `userAgent` | `digua-http-client/0.2.0` | UA header |

## 与其它 SDK 的关系

- 此模块**只做 HTTP 透传** —— 不解析响应 JSON，调用方拿到 `Response.body` 字符串自行处理（用 Jackson / Gson 等）
- 需要做完整业务模型映射（强类型 `User` / `Role` 等）请用领域 SDK：`digua-user-user-sdk` / `digua-token-user-sdk` / 等
- 适合用于尚未有专门 SDK 的下游服务，或者只需要一两个端点的轻量集成

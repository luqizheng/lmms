# Java 接入指南

> 用 Java 应用接入本微服务系统（api-gateway + iam-service + token-service + 其它业务服务）的完整说明。
> 最近更新：2026-06-26

本文档目标读者：

- 在另一个 Java 应用（Spring Boot、纯 Java 后端、批处理工具等）里**作为客户端**调用本平台 API 的开发者
- 想用 Java 直接对接 `api-gateway` 暴露的 HTTP 接口（登录、拿 JWT、CRUD 资源）
- 想理解 W3C TraceContext 跨服务串联在 Java 侧怎么打的同学

> ℹ️ **关于 `sdk/java/`**：`0.2.0` 起 Java SDK 已对齐当前 `api-gateway` 的 v1 路由（`/v1/iam/...`、`/v1/tokens/...`、`/v1/api-keys`），登录请求体改为 JSON。直接用 SDK 即可，不必再按本文示例手写 HTTP。详见文末「§ 9 SDK 现状」。

---

## 1. 平台总览

```
┌────────────────────────────────────────────────────────┐
│ 你的 Java 应用 (Spring Boot / 纯 Java)                  │
└──────────────┬─────────────────────────────────────────┘
               │  HTTPS (统一 baseUrl)
               ▼
        ┌─────────────────┐
        │  api-gateway    │  :3000
        │  JWT 校验 / 限流 │
        │  路由 / Trace 注入│
        └────────┬────────┘
                 │ (Consul 服务发现)
   ┌─────────────┼─────────────────────────────┐
   ▼             ▼             ▼               ▼
┌─────────┐ ┌──────────┐ ┌──────────────┐ ┌──────────┐
│ iam-svc │ │token-svc │ │notification  │ │ audit / │
│ :6002   │ │ :6001    │ │ :6003        │ │ file ... │
└─────────┘ └──────────┘ └──────────────┘ └──────────┘
```

**重要约定**：

- 所有外部调用统一走 `api-gateway` 的**单一 baseUrl**（生产环境一般是 `https://gw.example.com`，本地是 `http://localhost:3000`）。
- 不要直接调下游服务端口（6001–6007）—— 那是内网端口，外部不可达且不带网关的限流、追踪、JWT 校验。
- 所有受保护接口要求 `Authorization: Bearer <jwt>`。JWT 由 iam-service 签发，所有下游服务都从这个 header 解析身份（不验签名，仅 decode payload）。
- 每次响应都带 `X-Trace-Id` header，用于跨服务排障。

---

## 2. 端点速查

> 完整路由见 `infrastructure/config/api-gateway/routes/*.toml`。

### 2.1 认证（公开，无需 token）

| 方法 | 路径 | 用途 |
|------|------|------|
| POST | `/v1/iam/auth/login` | 用户名密码登录，拿 `access_token` + `refresh_token` |
| POST | `/v1/iam/auth/refresh` | 用 refresh_token 续 token |
| POST | `/v1/iam/auth/register` | 注册新用户 |
| GET  | `/v1/iam/auth/setup-status` | 系统是否已完成初始化 |

### 2.2 用户 / 角色 / 权限 / 组织（受保护）

| 方法 | 路径 | 用途 |
|------|------|------|
| GET / POST | `/v1/iam/users` | 列表 / 创建 |
| GET / PUT / DELETE | `/v1/iam/users/{id}` | 详情 / 改 / 删 |
| GET / POST | `/v1/iam/roles` | 角色 |
| PUT | `/v1/iam/roles/{id}/permissions` | 给角色绑权限 |
| GET / POST | `/v1/iam/permissions` | 权限 |
| GET / POST | `/v1/iam/org-units` | 组织（树形） |
| GET | `/v1/iam/sessions` | 当前用户的会话列表 |

### 2.3 Token / OAuth Client（部分公开）

| 方法 | 路径 | 用途 | 公开？ |
|------|------|------|--------|
| POST | `/v1/tokens/access` | 服务间 client_credentials 换 token | 是 |
| POST | `/v1/tokens/refresh` | 刷新 token | 是 |
| POST | `/v1/tokens/revoke` | 吊销 token | 是 |
| GET  | `/v1/tokens/.well-known/jwks.json` | 公钥（验签用） | 是 |
| GET / POST | `/v1/admin/oauth-clients` | 管理 OAuth client | 否 |
| GET / POST | `/v1/api-keys` | 管理 API Key | 否 |

### 2.4 业务服务

| 路径前缀 | 服务 | 备注 |
|----------|------|------|
| `/v1/notification/**` | notification-service | `/v1/notification/ws` 是 WebSocket |
| `/v1/audit/**` | audit-service | |
| `/v1/pure/**` | prue_backend | 前端菜单等 |

健康检查端点（公开）：`/v1/iam/health`、`/v1/audit/health`、`/v1/notification/health`、`/v1/pure/health` 等。

### 2.5 公共响应约定

- 鉴权失败：`401`，body `{"code": "...", "message": "..."}`
- 限流：`429` + `Retry-After: 1`（详见 `docs/user-manual/07-rate-limiting.md`）
- 校验失败：`400` / `422`
- 服务内部错误：`5xx`
- 所有响应都会带 `X-Trace-Id: <32-hex>`，排障时把它放到 Jaeger UI 直接搜

---

## 3. 鉴权流程

### 3.1 终端用户（username + password）

```
1. POST /v1/iam/auth/login        →  返回 { access_token, refresh_token, expires_in, ... }
2. 把 access_token 存起来（推荐内存 + 提前 60s 主动刷新）
3. 调业务接口时带 Authorization: Bearer <access_token>
4. access_token 快过期或返回 401 时：
   POST /v1/iam/auth/refresh { "refresh_token": "..." }  →  返回新一对 token
```

**登录请求体格式（必须是 JSON）**：

```json
POST /v1/iam/auth/login
Content-Type: application/json

{
  "username": "alice",
  "password": "P@ssw0rd!"
}
```

**响应**：

```json
HTTP/1.1 200 OK
X-Trace-Id: 0af7651916cd43dd8448eb211c80319c

{
  "access_token": "eyJhbGc...",
  "refresh_token": "...",
  "token_type": "Bearer",
  "expires_in": 3600,
  "user": {
    "id": "u-001",
    "username": "alice",
    "nickname": "Alice",
    "roles": ["admin"],
    "permissions": ["iam:user:read", ...]
  }
}
```

### 3.2 服务间（client_credentials）

如果你的 Java 应用是个**后端服务**，不是某个具体用户在调用，那应当走 client_credentials：

```
POST /v1/tokens/access
Content-Type: application/x-www-form-urlencoded

grant_type=client_credentials&client_id=<id>&client_secret=<secret>
```

返回结构同上（无 refresh_token）。提前向平台管理员申请 client_id / secret，存到你应用的安全配置中（环境变量或 vault）。

### 3.3 Token 校验（自己解析 JWT）

下游服务约定为**不验签名**、只 decode payload 取 `sub` / `roles` / `permissions`（见 `libs/jwt-ctx/src/principal.rs`）。但你**作为客户端**如果想本地做"token 是否快过期"判断，只需要解 payload 的 `exp` 字段即可；要做强校验则用 `/v1/tokens/.well-known/jwks.json` 拿公钥跑 RS256 验签。

---

## 4. 接入方式 A：原生 `java.net.http.HttpClient`（推荐）

适用 Java 11+。零依赖，足够覆盖 95% 的场景。

### 4.1 Maven 依赖

不需要额外依赖，只用 JDK 自带的 `java.net.http` 和任意 JSON 库（这里用 Jackson）：

```xml
<dependency>
  <groupId>com.fasterxml.jackson.core</groupId>
  <artifactId>jackson-databind</artifactId>
  <version>2.16.1</version>
</dependency>
```

### 4.2 完整示例：登录 + 调业务接口

```java
package example;

import com.fasterxml.jackson.databind.JsonNode;
import com.fasterxml.jackson.databind.ObjectMapper;
import com.fasterxml.jackson.databind.node.ObjectNode;

import java.net.URI;
import java.net.http.HttpClient;
import java.net.http.HttpRequest;
import java.net.http.HttpResponse;
import java.time.Duration;

public class PlatformClient {
    private static final ObjectMapper M = new ObjectMapper();

    private final String baseUrl;
    private final HttpClient http;
    private volatile String accessToken;
    private volatile long expiresAtMs;

    public PlatformClient(String baseUrl) {
        this.baseUrl = baseUrl;
        this.http = HttpClient.newBuilder()
                .connectTimeout(Duration.ofSeconds(5))
                .version(HttpClient.Version.HTTP_2)
                .build();
    }

    /** username/password 登录 */
    public void login(String username, String password) throws Exception {
        ObjectNode body = M.createObjectNode()
                .put("username", username)
                .put("password", password);

        HttpRequest req = HttpRequest.newBuilder()
                .uri(URI.create(baseUrl + "/v1/iam/auth/login"))
                .header("Content-Type", "application/json")
                .header("Accept", "application/json")
                .timeout(Duration.ofSeconds(15))
                .POST(HttpRequest.BodyPublishers.ofString(M.writeValueAsString(body)))
                .build();

        HttpResponse<String> resp = http.send(req, HttpResponse.BodyHandlers.ofString());
        if (resp.statusCode() != 200) {
            throw new RuntimeException("login failed: HTTP " + resp.statusCode()
                    + " trace=" + resp.headers().firstValue("X-Trace-Id").orElse("-")
                    + " body=" + resp.body());
        }
        JsonNode json = M.readTree(resp.body());
        this.accessToken = json.get("access_token").asText();
        long expiresIn = json.path("expires_in").asLong(3600);
        // 提前 60s 主动刷新
        this.expiresAtMs = System.currentTimeMillis() + (expiresIn - 60) * 1000;
    }

    /** 任意受保护 GET 调用 */
    public JsonNode get(String path) throws Exception {
        ensureToken();
        HttpRequest req = HttpRequest.newBuilder()
                .uri(URI.create(baseUrl + path))
                .header("Authorization", "Bearer " + accessToken)
                .header("Accept", "application/json")
                .timeout(Duration.ofSeconds(15))
                .GET()
                .build();
        HttpResponse<String> resp = http.send(req, HttpResponse.BodyHandlers.ofString());
        if (resp.statusCode() == 401) {
            // 强制重新登录后再试一次
            this.accessToken = null;
            ensureToken();
            return get(path);  // 注意防重入：生产代码加重试上限
        }
        if (resp.statusCode() >= 400) {
            throw new RuntimeException("HTTP " + resp.statusCode()
                    + " trace=" + resp.headers().firstValue("X-Trace-Id").orElse("-")
                    + " body=" + resp.body());
        }
        return M.readTree(resp.body());
    }

    private void ensureToken() {
        if (accessToken == null || System.currentTimeMillis() >= expiresAtMs) {
            throw new IllegalStateException("not logged in or token expired; call login() first");
        }
    }

    // 使用示例
    public static void main(String[] args) throws Exception {
        PlatformClient c = new PlatformClient("http://localhost:3000");
        c.login("alice", "P@ssw0rd!");

        JsonNode users = c.get("/v1/iam/users?page=1&size=20");
        System.out.println(users.toPrettyString());

        JsonNode me = c.get("/v1/iam/sessions");
        System.out.println(me.toPrettyString());
    }
}
```

### 4.3 处理 429 限流

平台对登录、注册等敏感端点限流较严（5 req/s）。建议加退避：

```java
if (resp.statusCode() == 429) {
    long retryAfter = resp.headers().firstValueAsLong("Retry-After").orElse(1L);
    Thread.sleep(retryAfter * 1000);
    // 重试
}
```

详见 `docs/user-manual/07-rate-limiting.md`。

---

## 5. 接入方式 B：Spring Boot + `RestTemplate` / `WebClient`

适合已经在用 Spring 的项目。

### 5.1 配置

`application.yml`：

```yaml
platform:
  base-url: http://localhost:3000     # 生产环境改成 https://gw.example.com
  username: ${PLATFORM_USER:alice}
  password: ${PLATFORM_PASS:}
  connect-timeout: 5s
  read-timeout: 15s
```

### 5.2 RestTemplate 版本（同步）

```java
@Configuration
public class PlatformConfig {
    @Bean
    public RestTemplate platformRestTemplate(RestTemplateBuilder builder) {
        return builder
                .setConnectTimeout(Duration.ofSeconds(5))
                .setReadTimeout(Duration.ofSeconds(15))
                .additionalInterceptors(new BearerTokenInterceptor())
                .build();
    }
}

@Component
public class BearerTokenInterceptor implements ClientHttpRequestInterceptor {
    private final TokenStore store;
    public BearerTokenInterceptor(TokenStore store) { this.store = store; }

    @Override
    public ClientHttpResponse intercept(HttpRequest req, byte[] body,
                                        ClientHttpRequestExecution exec) throws IOException {
        // 公共路径不加 token
        if (!req.getURI().getPath().startsWith("/v1/iam/auth/")
                && !req.getURI().getPath().startsWith("/v1/tokens/")) {
            req.getHeaders().setBearerAuth(store.accessToken());
        }
        ClientHttpResponse resp = exec.execute(req, body);
        if (resp.getStatusCode().value() == 401) {
            store.invalidate();
            req.getHeaders().setBearerAuth(store.accessToken());
            return exec.execute(req, body);
        }
        return resp;
    }
}

@Service
public class TokenStore {
    private final RestTemplate raw;   // 不走拦截器
    private final String baseUrl, user, pass;
    private volatile String token;
    private volatile long expiresAtMs;

    public synchronized String accessToken() {
        if (token == null || System.currentTimeMillis() >= expiresAtMs) {
            login();
        }
        return token;
    }

    public synchronized void invalidate() {
        token = null;
        expiresAtMs = 0;
    }

    private void login() {
        Map<String, String> body = Map.of("username", user, "password", pass);
        ResponseEntity<JsonNode> resp = raw.postForEntity(
                baseUrl + "/v1/iam/auth/login", body, JsonNode.class);
        JsonNode j = resp.getBody();
        this.token = j.get("access_token").asText();
        this.expiresAtMs = System.currentTimeMillis()
                + (j.path("expires_in").asLong(3600) - 60) * 1000;
    }
}
```

### 5.3 WebClient 版本（异步 / 响应式）

```java
@Bean
public WebClient platformWebClient(@Value("${platform.base-url}") String baseUrl,
                                   TokenStore store) {
    return WebClient.builder()
            .baseUrl(baseUrl)
            .filter((req, next) -> {
                if (req.url().getPath().startsWith("/v1/iam/auth/")
                        || req.url().getPath().startsWith("/v1/tokens/")) {
                    return next.exchange(req);
                }
                ClientRequest auth = ClientRequest.from(req)
                        .header("Authorization", "Bearer " + store.accessToken())
                        .build();
                return next.exchange(auth).flatMap(resp -> {
                    if (resp.statusCode().value() == 401) {
                        store.invalidate();
                        ClientRequest retry = ClientRequest.from(req)
                                .header("Authorization", "Bearer " + store.accessToken())
                                .build();
                        return next.exchange(retry);
                    }
                    return Mono.just(resp);
                });
            })
            .build();
}

// 使用
public Mono<JsonNode> listUsers() {
    return webClient.get()
            .uri("/v1/iam/users?page=1&size=20")
            .retrieve()
            .bodyToMono(JsonNode.class);
}
```

---

## 6. 接入方式 C：OkHttp

OkHttp 是最常见的 Java HTTP 库之一，适合 Android 客户端或追求小依赖的服务端项目。

```xml
<dependency>
  <groupId>com.squareup.okhttp3</groupId>
  <artifactId>okhttp</artifactId>
  <version>4.12.0</version>
</dependency>
```

```java
public class OkHttpPlatformClient {
    private final OkHttpClient client = new OkHttpClient.Builder()
            .connectTimeout(5, TimeUnit.SECONDS)
            .readTimeout(15, TimeUnit.SECONDS)
            .addInterceptor(this::auth)
            .build();
    private final ObjectMapper M = new ObjectMapper();
    private final String baseUrl;
    private volatile String token;

    public OkHttpPlatformClient(String baseUrl) { this.baseUrl = baseUrl; }

    public void login(String user, String pass) throws IOException {
        String body = M.writeValueAsString(Map.of("username", user, "password", pass));
        Request req = new Request.Builder()
                .url(baseUrl + "/v1/iam/auth/login")
                .post(RequestBody.create(body, MediaType.parse("application/json")))
                .build();
        try (Response resp = client.newBuilder()
                .interceptors().isEmpty() ? client : client.newBuilder().build()) {
            // 登录不走 auth interceptor —— 直接同步调用即可
        }
        try (Response resp = new OkHttpClient().newCall(req).execute()) {
            if (!resp.isSuccessful()) {
                throw new IOException("login HTTP " + resp.code() + " trace="
                        + resp.header("X-Trace-Id"));
            }
            JsonNode j = M.readTree(resp.body().string());
            this.token = j.get("access_token").asText();
        }
    }

    private Response auth(Interceptor.Chain chain) throws IOException {
        Request original = chain.request();
        if (token == null || original.url().encodedPath().startsWith("/v1/iam/auth/")) {
            return chain.proceed(original);
        }
        Request authed = original.newBuilder()
                .header("Authorization", "Bearer " + token)
                .build();
        return chain.proceed(authed);
    }

    public String get(String path) throws IOException {
        Request req = new Request.Builder().url(baseUrl + path).get().build();
        try (Response resp = client.newCall(req).execute()) {
            if (!resp.isSuccessful()) {
                throw new IOException("HTTP " + resp.code() + " trace=" + resp.header("X-Trace-Id"));
            }
            return resp.body().string();
        }
    }
}
```

---

## 7. 跨服务链路追踪（W3C TraceContext）

如果你的 Java 应用本身已经接了 OpenTelemetry，那调本平台时 Java 侧的出站请求会自动注入 `traceparent` header，api-gateway 会延续 trace，整条链路（Java → gateway → iam → token）都会出现在同一个 Jaeger trace 里。

详见 `docs-site/guides/otel-tracing.md` § 5（跨服务串联机制）。

最小启用方式（OpenTelemetry Java agent）：

```bash
java -javaagent:opentelemetry-javaagent.jar \
     -Dotel.service.name=my-java-app \
     -Dotel.exporter.otlp.endpoint=http://otel-collector:4317 \
     -jar my-app.jar
```

agent 会自动给 `java.net.http.HttpClient` / OkHttp / RestTemplate / WebClient 加 traceparent 注入，不需要改业务代码。

排查问题时，从响应 header 拿 `X-Trace-Id`，到 `http://localhost:16686/trace/<trace-id>` 直接打开看完整链路即可。

---

## 8. 常见问题

| 现象 | 可能原因 | 处理 |
|------|----------|------|
| 登录 `415 Unsupported Media Type` | Content-Type 不是 `application/json` | 改成 JSON body，不要用 form-urlencoded |
| 登录 `404 Not Found` | URL 用了旧的 `/api/v1/auth/login` 或 `/api/auth/login` | 改成 `/v1/iam/auth/login` |
| 调业务接口 `401 Unauthorized` | token 未带 / 已过期 / 格式错 | 确保 `Authorization: Bearer <token>`；过期就 `/v1/iam/auth/refresh` |
| `429 Too Many Requests` | 命中网关限流 | 看 `Retry-After`，退避后重试；详见 `docs/user-manual/07-rate-limiting.md` |
| 调用偶发 `502 / 504` | 下游服务被 Consul 暂时摘除 | 加重试（指数退避，2–3 次） |
| 想看完整调用链路 | — | 拿响应里的 `X-Trace-Id`，到 Jaeger UI（http://localhost:16686）搜 |
| 想本地起完整平台联调 | — | `docker compose -f infrastructure/docker-compose.yml up -d`，详见 `README.md` 「快速开始」 |
| 连接超时（本地） | 基础设施没启动 | 跑 `pnpm run check:env` 或看 `docker ps` |

---

## 9. SDK 现状

`sdk/java/` 共 6 个 Maven 模块（`common` / `user-service-user-sdk` / `user-service-client-sdk` / `token-server-user-sdk` / `token-server-client-sdk` / `kudi-http-client`）。**`0.2.0` 起已对齐当前 `api-gateway` 的 v1 路由与 JSON 请求体**，可以正式用于新接入。

### 9.1 已对齐

- 登录路径：`/v1/iam/auth/login`，body 为 JSON `{"username":"...","password":"..."}`，`Content-Type: application/json`。
- `client_credentials`：`/v1/tokens/access`，JSON body `{grant_type, code, client_id, client_secret}`。
- 用户/角色/权限：`/v1/iam/users[/...]`、`/v1/iam/roles`、`/v1/iam/permissions`。
- token 端点：`/v1/tokens/access`、`/v1/tokens/refresh`、`/v1/tokens/revoke`、`/v1/tokens/resource`、`/v1/tokens/.well-known/jwks.json`、`/v1/api-keys`。
- 请求/响应 DTO 与 token-service 实际签名一致（`revoke {token_jti, expires_in}`、`resource {resource_type, resource_id, permission, ttl_sec?}`、`api-keys {name, permissions, grant_types}`，响应 `{api_key_id, api_key_secret}` 落到新的 `ApiKeyInfo`）。

### 9.2 快速接入示例

```java
import com.kudi.coder.sdk.user.UserServiceUserClient;
import com.kudi.coder.sdk.user.User;

try (UserServiceUserClient c = UserServiceUserClient.login(
        "https://gw.example.com", "alice", "P@ssw0rd!")) {
    User me = c.getById("u-001");
    System.out.println(me.getUsername());
}
```

### 9.3 命名遗留

artifact ID 仍是历史命名（`user-service-*` / `token-server-*`），但内部已经打通的是合并后的 `iam-service` 和 `token-service`。后续大版本再考虑 artifact 重命名（破坏性，单独走升级）。

### 9.4 本地构建（无需安装 JDK）

仓库提供了 Docker 化的 Java SDK 构建脚本，行为对齐 Rust 服务的 `scripts/run-tests-docker.js` —— 主机零 JDK / Maven 依赖：

```bash
# 默认 verify（编译 + 测试 + 打包），首次会自动 docker build builder 镜像
node scripts/build-java-sdk-docker.js

# 仅跑测试
node scripts/build-java-sdk-docker.js test

# 仅编译 / 打包 / 部署
node scripts/build-java-sdk-docker.js compile
node scripts/build-java-sdk-docker.js package
node scripts/build-java-sdk-docker.js deploy

# 切 maven profile（java8 默认 / java17 / java21）
node scripts/build-java-sdk-docker.js test --profile java17

# 透传 mvn 参数
node scripts/build-java-sdk-docker.js test -- -pl user-service-user-sdk -am
```

详见 `sdk/java/README.md` 「编译」节。

---

## 10. 参考

- `README.md` — 平台总览与本地开发
- `docs/user-manual/` — 完整用户手册（含 § 07 限流）
- `docs-site/guides/api-gateway-routing.md` — 网关路由配置详解
- `docs-site/guides/observability.md` — 可观测性
- `docs-site/guides/otel-tracing.md` — Java 应用接入 OTel 跨服务串联
- `infrastructure/config/api-gateway/routes/*.toml` — 当前生效的真实路由清单
- `services/iam-service/openapi.yaml` — IAM 服务 OpenAPI 规范（用于代码生成参考）
- `services/token-service/openapi.yaml` — token-service OpenAPI 规范
- `sdk/openapi/` — 离线 OpenAPI 文档

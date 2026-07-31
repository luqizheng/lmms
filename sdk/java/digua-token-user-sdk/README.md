# digua-token-user-sdk

终端用户访问 `token-service` 的 SDK：先经 `iam-service` 登录拿 JWT，再用同一 JWT 调用 token 相关接口（刷新 / 撤销 / 资源令牌 / JWKS）。走 v1 路由 `/v1/iam/auth/login` + `/v1/tokens/...`。

## 安装

```xml
<dependency>
  <groupId>io.digua</groupId>
  <artifactId>digua-token-user-sdk</artifactId>
  <version>0.2.0</version>
</dependency>
```

## 用法

```java
TokenServerUserClient c = TokenServerUserClient.login(
    "https://gw.example.com", "alice", "secret");

TokenInfo t = c.exchangeCode("auth-code", "client-id", "client-secret");
TokenInfo t2 = c.refresh(t.getRefreshToken(), "client-id", "client-secret");

// revoke by JWT id
c.revoke("jti-xxx", 3600);

// issue a resource token
ResourceTokenInfo rt = c.issueResource("doc", "doc-123", "read", 600L);
```

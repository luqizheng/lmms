# token-server-client-sdk

服务之间或受信任后台访问 `token-service` 的 SDK（`client_credentials` 流程），走 v1 路由 `/v1/tokens/...` 和 `/v1/api-keys`。

## 安装

```xml
<dependency>
  <groupId>com.kudi.coder</groupId>
  <artifactId>token-server-client-sdk</artifactId>
  <version>0.2.0</version>
</dependency>
```

## 用法

```java
TokenServerClientClient cc = TokenServerClientClient.create(
    "https://gw.example.com", "api_key_xxx", "secret_yyy", "auth-code");

ApiKeyInfo key = cc.createApiKey(
    "backend-x",
    java.util.Arrays.asList("notification.admin"),
    java.util.Arrays.asList("client_credentials"));
System.out.println(key.getApiKeyId() + " / " + key.getApiKeySecret());
```

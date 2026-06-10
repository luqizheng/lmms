# token-server-client-sdk

服务之间或受信任后台访问 token-server 的 SDK（client_credentials 流程）。

## 用法

```java
TokenServerClientClient cc = TokenServerClientClient.create(
    "https://gw.kudi.repos", "api_key_xxx", "secret_yyy", "auth-code");
ApiKeyInfo key = cc.createApiKey("backend-x", "client-id", "client-secret");
```

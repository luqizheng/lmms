# token-server-user-sdk

终端用户访问 token-server 的 SDK：登录、刷新、撤销、资源令牌、JWKS。

## 用法

```java
TokenServerUserClient c = TokenServerUserClient.login(
    "https://gw.kudi.repos", "alice", "secret");
TokenInfo t = c.login("auth-code", "client-id", "client-secret");
```

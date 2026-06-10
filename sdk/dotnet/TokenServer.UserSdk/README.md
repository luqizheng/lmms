# KudiCoder.TokenServer.UserSdk

终端用户访问 token-server 的 .NET SDK。

```csharp
var c = TokenServerUserClient.Login("https://gw.kudi.repos", "alice", "secret");
var t = await c.LoginAsync("auth-code", "client-id", "client-secret");
```

# KudiCoder.TokenServer.ClientSdk

服务之间访问 token-server 的 .NET SDK（client_credentials）。

```csharp
var cc = TokenServerClientClient.Create("https://gw.kudi.repos", "api_key_xxx", "secret_yyy", "auth-code");
var key = await cc.CreateApiKeyAsync("backend-x", "client-id", "client-secret");
```

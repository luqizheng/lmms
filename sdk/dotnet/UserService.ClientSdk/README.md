# KudiCoder.UserService.ClientSdk

服务之间访问 user-service 的 .NET SDK（client_credentials）。

```csharp
var cc = UserServiceClientClient.Create("https://gw.kudi.repos", "api_key_xxx", "secret_yyy", "auth-code");
var all = await cc.ListAsync();
```

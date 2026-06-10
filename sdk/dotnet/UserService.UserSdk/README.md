# KudiCoder.UserService.UserSdk

终端用户访问 user-service 的 .NET SDK。

```csharp
var c = UserServiceUserClient.Login("https://gw.kudi.repos", "alice", "secret");
var u = await c.GetByIdAsync("u-1");
```

using KudiCoder.Sdk.Common;
using KudiCoder.UserService.UserSdk;
using KudiCoder.UserService.ClientSdk;
using KudiCoder.TokenServer.UserSdk;
using KudiCoder.TokenServer.ClientSdk;
using Xunit;

namespace KudiCoder.Sdk.Tests;

/// <summary>
/// 烟雾测试：只验证 4 个业务 SDK 静态工厂方法能调用并返回实例，
/// 不会触发真实 HTTP（构造函数是惰性求 token）。
/// 完整业务测试待 HttpClient 注入改造后补全。
/// </summary>
public class SdkEndToEndSmoke
{
    [Fact]
    public void UserServiceUserClientFactoryWorks()
    {
        var c = UserServiceUserClient.Login("https://gw.example", "alice", "secret");
        Assert.NotNull(c);
        Assert.Equal("https://gw.example", c.GetConfig().BaseUrl);
    }

    [Fact]
    public void UserServiceClientClientFactoryWorks()
    {
        var c = UserServiceClientClient.Create("https://gw.example", "ak", "sec", "code");
        Assert.NotNull(c);
    }

    [Fact]
    public void TokenServerUserClientFactoryWorks()
    {
        var c = TokenServerUserClient.Login("https://gw.example", "alice", "secret");
        Assert.NotNull(c);
    }

    [Fact]
    public void TokenServerClientClientFactoryWorks()
    {
        var c = TokenServerClientClient.Create("https://gw.example", "ak", "sec", "code");
        Assert.NotNull(c);
    }
}

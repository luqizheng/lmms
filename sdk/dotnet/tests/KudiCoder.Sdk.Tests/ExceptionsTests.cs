using KudiCoder.Sdk.Common;
using Xunit;

namespace KudiCoder.Sdk.Tests;

public class ExceptionsTests
{
    [Fact]
    public void AuthExceptionHasStatus401()
    {
        var e = new AuthException("nope", 401, "details");
        Assert.Equal(401, e.StatusCode);
        Assert.Equal("details", e.Details);
    }

    [Fact]
    public void RateLimitExceptionHasStatus429()
    {
        var e = new RateLimitException("slow", "details");
        Assert.Equal(429, e.StatusCode);
    }

    [Fact]
    public void ServerExceptionDefaultStatusIs500()
    {
        var e = new ServerException("boom");
        Assert.Equal(500, e.StatusCode);
    }
}

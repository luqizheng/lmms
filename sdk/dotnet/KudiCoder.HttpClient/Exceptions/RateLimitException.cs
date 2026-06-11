namespace KudiCoder.HttpClient.Exceptions;

public sealed class RateLimitException : KudiHttpException
{
    public RateLimitException(string message) : base(message, 429, null, null) { }
    public RateLimitException(string message, string? details) : base(message, 429, details, null) { }
}

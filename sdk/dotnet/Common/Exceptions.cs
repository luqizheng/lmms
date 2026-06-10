namespace KudiCoder.Sdk.Common;

public class SdkException : Exception
{
    public int StatusCode { get; }
    public string? Details { get; }

    public SdkException(string message) : this(message, 0, null, null) { }
    public SdkException(string message, Exception? inner) : this(message, 0, null, inner) { }
    public SdkException(string message, int statusCode, string? details, Exception? inner)
        : base(message, inner)
    {
        StatusCode = statusCode;
        Details = details;
    }
}

public sealed class AuthException : SdkException
{
    public AuthException(string message) : base(message, 401, null, null) { }
    public AuthException(string message, int statusCode, string? details)
        : base(message, statusCode, details, null) { }
}

public sealed class RateLimitException : SdkException
{
    public RateLimitException(string message) : base(message, 429, null, null) { }
    public RateLimitException(string message, string? details)
        : base(message, 429, details, null) { }
}

public sealed class ValidationException : SdkException
{
    public ValidationException(string message) : base(message, 400, null, null) { }
    public ValidationException(string message, int statusCode, string? details)
        : base(message, statusCode, details, null) { }
}

public sealed class ServerException : SdkException
{
    public ServerException(string message) : base(message, 500, null, null) { }
    public ServerException(string message, int statusCode, string? details)
        : base(message, statusCode, details, null) { }
}

public sealed class NetworkException : SdkException
{
    public NetworkException(string message) : base(message, 0, null, null) { }
    public NetworkException(string message, Exception? inner) : base(message, 0, null, inner) { }
}

namespace KudiCoder.HttpClient.Exceptions;

public sealed class AuthException : KudiHttpException
{
    public AuthException(string message) : base(message, 401, null, null) { }
    public AuthException(string message, int statusCode, string? details) : base(message, statusCode, details, null) { }
}

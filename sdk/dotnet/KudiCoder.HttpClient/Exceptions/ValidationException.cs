namespace KudiCoder.HttpClient.Exceptions;

public sealed class ValidationException : KudiHttpException
{
    public ValidationException(string message) : base(message, 400, null, null) { }
    public ValidationException(string message, int statusCode, string? details) : base(message, statusCode, details, null) { }
}

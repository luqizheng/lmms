namespace KudiCoder.HttpClient.Exceptions;

public sealed class ServerException : KudiHttpException
{
    public ServerException(string message) : base(message, 500, null, null) { }
    public ServerException(string message, int statusCode, string? details) : base(message, statusCode, details, null) { }
}

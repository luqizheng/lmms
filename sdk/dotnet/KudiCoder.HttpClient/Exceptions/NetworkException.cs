namespace KudiCoder.HttpClient.Exceptions;

public sealed class NetworkException : KudiHttpException
{
    public NetworkException(string message) : base(message, 0, null, null) { }
    public NetworkException(string message, Exception? inner) : base(message, 0, null, inner) { }
}

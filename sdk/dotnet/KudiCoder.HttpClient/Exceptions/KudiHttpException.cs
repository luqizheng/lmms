namespace KudiCoder.HttpClient.Exceptions;

public class KudiHttpException : Exception
{
    public int StatusCode { get; }
    public string? Details { get; }

    public KudiHttpException(string message) : this(message, 0, null, null) { }
    public KudiHttpException(string message, Exception? inner) : this(message, 0, null, inner) { }
    public KudiHttpException(string message, int statusCode, string? details, Exception? inner)
        : base(message, inner)
    {
        StatusCode = statusCode;
        Details = details;
    }
}

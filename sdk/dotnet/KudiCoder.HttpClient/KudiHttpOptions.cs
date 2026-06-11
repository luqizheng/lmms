namespace KudiCoder.HttpClient;

public class KudiHttpOptions
{
    public string TokenEndpoint { get; set; } = "http://token-svc/api/tokens/access";
    public int ConnectTimeoutMs { get; set; } = 5000;
    public int ReadTimeoutMs { get; set; } = 15000;
    public int RetryCount { get; set; } = 1;
    public TimeSpan RefreshSkew { get; set; } = TimeSpan.FromSeconds(60);
    public string UserAgent { get; set; } = "kudi-http-client/0.1.0";
}

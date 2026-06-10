namespace KudiCoder.Sdk.Common;

public class SdkConfig
{
    public string BaseUrl { get; set; } = "";
    public int ConnectTimeoutMs { get; set; } = 5000;
    public int ReadTimeoutMs { get; set; } = 15000;
    public int RetryCount { get; set; } = 1;
    public string UserAgent { get; set; } = "kudi-coder-sdk/0.1.0";

    public SdkConfig() { }

    public SdkConfig(string baseUrl)
    {
        BaseUrl = baseUrl;
    }
}

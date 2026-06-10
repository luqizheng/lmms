using System.Text.Json;
using System.Text.Json.Serialization;

namespace KudiCoder.Sdk.Common;

public static class JsonUtil
{
    public static readonly JsonSerializerOptions Options = new()
    {
        PropertyNamingPolicy = JsonNamingPolicy.CamelCase,
        DefaultIgnoreCondition = JsonIgnoreCondition.WhenWritingNull,
        WriteIndented = false
    };

    public static string ToJson(object value) => JsonSerializer.Serialize(value, Options);

    public static T? FromJson<T>(string body) => JsonSerializer.Deserialize<T>(body, Options);
}

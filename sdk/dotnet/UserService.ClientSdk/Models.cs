using System.Text.Json.Serialization;
using KudiCoder.Sdk.Common;

namespace KudiCoder.UserService.ClientSdk;

public class User
{
    [JsonPropertyName("id")] public string? Id { get; set; }
    [JsonPropertyName("username")] public string? Username { get; set; }
    [JsonPropertyName("email")] public string? Email { get; set; }
    [JsonPropertyName("status")] public string? Status { get; set; }
    [JsonPropertyName("roles")] public List<string>? Roles { get; set; }
    [JsonPropertyName("permissions")] public List<string>? Permissions { get; set; }
}

public class Role
{
    [JsonPropertyName("id")] public string? Id { get; set; }
    [JsonPropertyName("name")] public string? Name { get; set; }
    [JsonPropertyName("description")] public string? Description { get; set; }
    [JsonPropertyName("permissions")] public List<string>? Permissions { get; set; }
}

public class Permission
{
    [JsonPropertyName("id")] public string? Id { get; set; }
    [JsonPropertyName("code")] public string? Code { get; set; }
    [JsonPropertyName("description")] public string? Description { get; set; }
}

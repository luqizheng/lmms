using KudiCoder.Sdk.Common;

namespace KudiCoder.UserService.ClientSdk;

public class UserServiceClientClient : SdkClient
{
    private readonly string _basePath;

    private UserServiceClientClient(SdkConfig config, ICredentialProvider provider, string basePath)
        : base(config, provider) { _basePath = basePath; }

    public static UserServiceClientClient Create(string baseUrl, string apiKey, string secret, string code)
        => Create(new SdkConfig(baseUrl), apiKey, secret, code);

    public static UserServiceClientClient Create(SdkConfig config, string apiKey, string secret, string code)
        => new(config, new ClientCredentialsProvider(config, apiKey, secret, code), "/api");

    private string Url(string suffix) => Config.BaseUrl + _basePath + suffix;

    public async Task<User?> GetByIdAsync(string id, CancellationToken ct = default)
    {
        var r = await ExecuteWithAuthAsync(HttpRequest.Get(Url("/users/" + id)), ct);
        return MapBody<User>(r);
    }

    public async Task<List<User>> ListAsync(CancellationToken ct = default)
    {
        var r = await ExecuteWithAuthAsync(HttpRequest.Get(Url("/users")), ct);
        return JsonUtil.FromJson<List<User>>(r.Body) ?? new();
    }

    public async Task<User?> CreateAsync(object body, CancellationToken ct = default)
    {
        var r = await ExecuteWithAuthAsync(HttpRequest.Post(Url("/users")).WithJsonBody(body), ct);
        return MapBody<User>(r);
    }

    public async Task<User?> UpdateAsync(string id, object patch, CancellationToken ct = default)
    {
        var r = await ExecuteWithAuthAsync(HttpRequest.Put(Url("/users/" + id)).WithJsonBody(patch), ct);
        return MapBody<User>(r);
    }

    public async Task DeleteAsync(string id, CancellationToken ct = default)
    {
        var r = await ExecuteWithAuthAsync(HttpRequest.Delete(Url("/users/" + id)), ct);
        EnsureSuccess(r);
    }

    public async Task<List<Role>> ListRolesAsync(CancellationToken ct = default)
    {
        var r = await ExecuteWithAuthAsync(HttpRequest.Get(Url("/roles")), ct);
        return JsonUtil.FromJson<List<Role>>(r.Body) ?? new();
    }

    public async Task<Role?> CreateRoleAsync(object body, CancellationToken ct = default)
    {
        var r = await ExecuteWithAuthAsync(HttpRequest.Post(Url("/roles")).WithJsonBody(body), ct);
        return MapBody<Role>(r);
    }

    public async Task<List<Permission>> ListPermissionsAsync(CancellationToken ct = default)
    {
        var r = await ExecuteWithAuthAsync(HttpRequest.Get(Url("/permissions")), ct);
        return JsonUtil.FromJson<List<Permission>>(r.Body) ?? new();
    }
}

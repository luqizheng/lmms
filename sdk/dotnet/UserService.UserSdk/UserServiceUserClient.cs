using KudiCoder.Sdk.Common;

namespace KudiCoder.UserService.UserSdk;

public class UserServiceUserClient : SdkClient
{
    private readonly string _basePath;

    private UserServiceUserClient(SdkConfig config, ICredentialProvider provider, string basePath)
        : base(config, provider)
    {
        _basePath = basePath;
    }

    public static UserServiceUserClient Login(string baseUrl, string username, string password)
        => Login(new SdkConfig(baseUrl), username, password);

    public static UserServiceUserClient Login(SdkConfig config, string username, string password)
        => new(config, new PasswordCredentialProvider(config, username, password), "/api");

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

    public async Task<List<Permission>> ListPermissionsAsync(CancellationToken ct = default)
    {
        var r = await ExecuteWithAuthAsync(HttpRequest.Get(Url("/permissions")), ct);
        return JsonUtil.FromJson<List<Permission>>(r.Body) ?? new();
    }
}

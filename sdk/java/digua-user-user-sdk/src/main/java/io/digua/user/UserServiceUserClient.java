package io.digua.user;

import io.digua.common.SdkClient;
import io.digua.common.SdkConfig;
import io.digua.auth.CredentialProvider;
import io.digua.auth.PasswordCredentialProvider;
import io.digua.http.HttpRequest;
import io.digua.http.HttpResponse;

import java.util.List;
import java.util.Map;
import java.util.concurrent.CompletableFuture;

/**
 * user → iam-service SDK。
 * 用 username/password 登录，缓存 access_token。
 */
public class UserServiceUserClient extends SdkClient {
    private final String basePath;

    private UserServiceUserClient(SdkConfig config, CredentialProvider provider, String basePath) {
        super(config, provider);
        this.basePath = basePath;
    }

    public static UserServiceUserClient login(String baseUrl, String username, String password) {
        SdkConfig cfg = new SdkConfig(baseUrl);
        CredentialProvider p = new PasswordCredentialProvider(cfg, username, password);
        return new UserServiceUserClient(cfg, p, "/v1/iam");
    }

    public static UserServiceUserClient login(SdkConfig config, String username, String password) {
        CredentialProvider p = new PasswordCredentialProvider(config, username, password);
        return new UserServiceUserClient(config, p, "/v1/iam");
    }

    private String url(String suffix) {
        return config.getBaseUrl() + basePath + suffix;
    }

    public User getById(String id) {
        HttpResponse r = executeWithAuth(HttpRequest.get(url("/users/" + id)));
        return mapBody(r, User.class);
    }

    public CompletableFuture<User> getByIdAsync(String id) {
        return wrapAsync(() -> getById(id));
    }

    public List<User> list() {
        HttpResponse r = executeWithAuth(HttpRequest.get(url("/users")));
        return map(r.getBody(), User.class);
    }

    public CompletableFuture<List<User>> listAsync() {
        return wrapAsync(this::list);
    }

    public User update(String id, Map<String, Object> patch) {
        HttpResponse r = executeWithAuth(HttpRequest.put(url("/users/" + id)).jsonBody(patch));
        return mapBody(r, User.class);
    }

    public void delete(String id) {
        HttpResponse r = executeWithAuth(HttpRequest.delete(url("/users/" + id)));
        ensureSuccess(r);
    }

    public List<Role> listRoles() {
        HttpResponse r = executeWithAuth(HttpRequest.get(url("/roles")));
        return map(r.getBody(), Role.class);
    }

    public List<Permission> listPermissions() {
        HttpResponse r = executeWithAuth(HttpRequest.get(url("/permissions")));
        return map(r.getBody(), Permission.class);
    }

    private <T> List<T> map(String body, Class<T> element) {
        try {
            return io.digua.common.JsonUtil.mapper()
                    .readValue(body,
                            io.digua.common.JsonUtil.mapper().getTypeFactory()
                                    .constructCollectionType(java.util.List.class, element));
        } catch (Exception e) {
            throw new RuntimeException(e);
        }
    }
}

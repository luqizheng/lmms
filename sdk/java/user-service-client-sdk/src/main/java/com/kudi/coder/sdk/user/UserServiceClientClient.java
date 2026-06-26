package com.kudi.coder.sdk.user;

import com.kudi.coder.sdk.common.SdkClient;
import com.kudi.coder.sdk.common.SdkConfig;
import com.kudi.coder.sdk.common.auth.ClientCredentialsProvider;
import com.kudi.coder.sdk.common.auth.CredentialProvider;
import com.kudi.coder.sdk.common.http.HttpRequest;
import com.kudi.coder.sdk.common.http.HttpResponse;

import java.util.List;
import java.util.Map;
import java.util.concurrent.CompletableFuture;

/**
 * client → iam-service SDK。
 * 用 api_key/secret（client_credentials）调用。
 */
public class UserServiceClientClient extends SdkClient {
    private final String basePath;

    private UserServiceClientClient(SdkConfig config, CredentialProvider provider, String basePath) {
        super(config, provider);
        this.basePath = basePath;
    }

    public static UserServiceClientClient create(String baseUrl, String apiKey, String secret, String code) {
        SdkConfig cfg = new SdkConfig(baseUrl);
        CredentialProvider p = new ClientCredentialsProvider(cfg, apiKey, secret, code);
        return new UserServiceClientClient(cfg, p, "/v1/iam");
    }

    public static UserServiceClientClient create(SdkConfig config, String apiKey, String secret, String code) {
        CredentialProvider p = new ClientCredentialsProvider(config, apiKey, secret, code);
        return new UserServiceClientClient(config, p, "/v1/iam");
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

    public User create(Map<String, Object> body) {
        HttpResponse r = executeWithAuth(HttpRequest.post(url("/users")).jsonBody(body));
        return mapBody(r, User.class);
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

    public Role createRole(Map<String, Object> body) {
        HttpResponse r = executeWithAuth(HttpRequest.post(url("/roles")).jsonBody(body));
        return mapBody(r, Role.class);
    }

    public List<Permission> listPermissions() {
        HttpResponse r = executeWithAuth(HttpRequest.get(url("/permissions")));
        return map(r.getBody(), Permission.class);
    }

    private <T> List<T> map(String body, Class<T> element) {
        try {
            return com.kudi.coder.sdk.common.JsonUtil.mapper()
                    .readValue(body,
                            com.kudi.coder.sdk.common.JsonUtil.mapper().getTypeFactory()
                                    .constructCollectionType(java.util.List.class, element));
        } catch (Exception e) {
            throw new RuntimeException(e);
        }
    }
}

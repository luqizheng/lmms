package io.digua.web;

import io.digua.model.ResourceTokenInfo;
import io.digua.shared.ApiResponse;
import io.digua.token.TokenServerClientClient;
import io.digua.token.TokenServerUserClient;
import io.digua.user.Role;
import io.digua.user.UserServiceClientClient;
import org.springframework.web.bind.annotation.GetMapping;
import org.springframework.web.bind.annotation.RequestMapping;
import org.springframework.web.bind.annotation.RestController;

import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.Map;

@RestController
@RequestMapping("/demo/perm")
public class PermissionDemoController {

    private final UserServiceClientClient userClient;
    private final TokenServerUserClient userTokenClient;
    private final TokenServerClientClient clientTokenClient;

    public PermissionDemoController(UserServiceClientClient userClient,
                                    TokenServerUserClient userTokenClient,
                                    TokenServerClientClient clientTokenClient) {
        this.userClient = userClient;
        this.userTokenClient = userTokenClient;
        this.clientTokenClient = clientTokenClient;
    }

    @GetMapping("/public")
    public ApiResponse<Map<String, String>> publicEndpoint() {
        return ApiResponse.ok(Collections.<String, String>singletonMap("message", "public, no auth needed"));
    }

    @GetMapping("/internal")
    public ApiResponse<Role> internal() {
        Map<String, Object> body = new LinkedHashMap<>();
        body.put("name", "TASK_INTERNAL_ADMIN");
        body.put("description", "demo role created via current user client SDK");
        return ApiResponse.ok(userClient.createRole(body));
    }

    @GetMapping("/resource")
    public ApiResponse<ResourceTokenInfo> resource() {
        return ApiResponse.ok(userTokenClient.issueResource("task", "42", "read", 600L));
    }

    @GetMapping("/client-jwks")
    public ApiResponse<String> clientJwks() {
        return ApiResponse.ok(clientTokenClient.jwks());
    }
}

package com.kudi.coder.demowebapp.web;

import com.kudi.coder.demo.shared.ApiResponse;
import com.kudi.coder.sdk.common.model.ResourceTokenInfo;
import com.kudi.coder.sdk.token.TokenServerUserClient;
import com.kudi.coder.sdk.user.Role;
import com.kudi.coder.sdk.user.UserServiceClientClient;
import org.springframework.web.bind.annotation.GetMapping;
import org.springframework.web.bind.annotation.RequestMapping;
import org.springframework.web.bind.annotation.RestController;

import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.Map;

@RestController
@RequestMapping("/demo/perm")
public class PermissionDemoController {

    private final UserServiceClientClient clientClient;
    private final TokenServerUserClient tokenClient;

    public PermissionDemoController(UserServiceClientClient clientClient,
                                    TokenServerUserClient tokenClient) {
        this.clientClient = clientClient;
        this.tokenClient = tokenClient;
    }

    @GetMapping("/public")
    public ApiResponse<Map<String, String>> publicEndpoint() {
        return ApiResponse.ok(Collections.<String, String>singletonMap("message", "public, no auth needed"));
    }

    @GetMapping("/internal")
    public ApiResponse<Role> internal() {
        Map<String, Object> body = new LinkedHashMap<>();
        body.put("name", "TASK_INTERNAL_ADMIN");
        body.put("description", "demo role created via client-sdk");
        Role role = clientClient.createRole(body);
        return ApiResponse.ok(role);
    }

    @GetMapping("/resource")
    public ApiResponse<ResourceTokenInfo> resource() {
        return ApiResponse.ok(tokenClient.issueResource("task", "42", "read", 600L));
    }
}

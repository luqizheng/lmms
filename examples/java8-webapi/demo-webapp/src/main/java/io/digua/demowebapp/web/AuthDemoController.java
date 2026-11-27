package io.digua.web;

import io.digua.model.TokenInfo;
import io.digua.shared.ApiResponse;
import io.digua.token.TokenServerUserClient;
import io.digua.user.User;
import io.digua.user.UserServiceUserClient;
import org.springframework.web.bind.annotation.GetMapping;
import org.springframework.web.bind.annotation.PostMapping;
import org.springframework.web.bind.annotation.RequestBody;
import org.springframework.web.bind.annotation.RequestMapping;
import org.springframework.web.bind.annotation.RestController;

import java.util.Collections;
import java.util.Map;

@RestController
@RequestMapping("/demo/auth")
public class AuthDemoController {

    private final UserServiceUserClient userClient;
    private final TokenServerUserClient tokenClient;

    public AuthDemoController(UserServiceUserClient userClient, TokenServerUserClient tokenClient) {
        this.userClient = userClient;
        this.tokenClient = tokenClient;
    }

    @GetMapping("/me")
    public ApiResponse<User> me() {
        return ApiResponse.ok(userClient.getById("me"));
    }

    @GetMapping("/jwks")
    public ApiResponse<String> jwks() {
        return ApiResponse.ok(tokenClient.jwks());
    }

    @PostMapping("/refresh")
    public ApiResponse<TokenInfo> refresh(@RequestBody Map<String, String> body) {
        TokenInfo info = tokenClient.refresh(
                body.get("refresh_token"),
                body.get("client_id"),
                body.get("client_secret"));
        return ApiResponse.ok(info);
    }

    @PostMapping("/revoke")
    public ApiResponse<Map<String, Object>> revoke(@RequestBody Map<String, Object> body) {
        String jti = (String) body.get("jti");
        Object raw = body.get("expires_in");
        long expiresIn = raw == null ? 0L : (raw instanceof Number ? ((Number) raw).longValue() : Long.valueOf(raw.toString()));
        tokenClient.revoke(jti, expiresIn);
        return ApiResponse.ok(Collections.<String, Object>singletonMap("revoked", jti));
    }
}
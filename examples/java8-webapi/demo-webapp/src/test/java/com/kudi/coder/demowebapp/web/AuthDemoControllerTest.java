package io.digua.web;

import io.digua.WebMvcConfig;
import io.digua.JwtAuthInterceptor;
import io.digua.PermissionAspect;
import io.digua.TokenInfo;
import io.digua.TokenServerUserClient;
import io.digua.User;
import io.digua.UserServiceUserClient;
import org.junit.jupiter.api.Test;
import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.boot.test.autoconfigure.web.servlet.WebMvcTest;
import org.springframework.boot.test.mock.mockito.MockBean;
import org.springframework.context.annotation.Import;
import org.springframework.test.web.servlet.MockMvc;

import static org.mockito.ArgumentMatchers.anyString;
import static org.mockito.Mockito.when;
import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.get;
import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.post;
import static org.springframework.test.web.servlet.result.MockMvcResultMatchers.jsonPath;
import static org.springframework.test.web.servlet.result.MockMvcResultMatchers.status;

@WebMvcTest(AuthDemoController.class)
@Import({JwtAuthInterceptor.class, PermissionAspect.class, WebMvcConfig.class})
class AuthDemoControllerTest {

    @Autowired private MockMvc mvc;
    @MockBean private UserServiceUserClient userClient;
    @MockBean private TokenServerUserClient tokenClient;

    @Test
    void me_returns_current_user() throws Exception {
        User u = new User();
        u.setId("me");
        u.setUsername("alice");
        when(userClient.getById("me")).thenReturn(u);

        mvc.perform(get("/demo/auth/me"))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.code").value("OK"))
                .andExpect(jsonPath("$.data.id").value("me"))
                .andExpect(jsonPath("$.data.username").value("alice"));
    }

    @Test
    void refresh_returns_new_access_token() throws Exception {
        TokenInfo t = new TokenInfo();
        t.setAccessToken("acc-2");
        t.setExpiresIn(3600L);
        when(tokenClient.refresh(anyString(), anyString(), anyString())).thenReturn(t);

        String body = "{\"refresh_token\":\"ref-1\",\"client_id\":\"cid\",\"client_secret\":\"sec\"}";
        mvc.perform(post("/demo/auth/refresh")
                        .header("X-User-Id", "alice")
                        .contentType("application/json")
                        .content(body))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.code").value("OK"))
                .andExpect(jsonPath("$.data.access_token").value("acc-2"))
                .andExpect(jsonPath("$.data.expires_in").value(3600));
    }
}

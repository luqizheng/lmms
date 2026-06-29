package com.kudi.coder.demowebapp.web;

import com.kudi.coder.demowebapp.config.WebMvcConfig;
import com.kudi.coder.demowebapp.security.JwtAuthInterceptor;
import com.kudi.coder.demowebapp.security.PermissionAspect;
import com.kudi.coder.sdk.common.model.ResourceTokenInfo;
import com.kudi.coder.sdk.token.TokenServerUserClient;
import com.kudi.coder.sdk.user.Role;
import com.kudi.coder.sdk.user.UserServiceClientClient;
import org.junit.jupiter.api.Test;
import org.mockito.ArgumentCaptor;
import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.boot.test.autoconfigure.web.servlet.WebMvcTest;
import org.springframework.boot.test.mock.mockito.MockBean;
import org.springframework.context.annotation.Import;
import org.springframework.test.web.servlet.MockMvc;

import java.util.Collections;
import java.util.Map;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.mockito.ArgumentMatchers.any;
import static org.mockito.ArgumentMatchers.anyLong;
import static org.mockito.ArgumentMatchers.anyString;
import static org.mockito.Mockito.verify;
import static org.mockito.Mockito.when;
import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.get;
import static org.springframework.test.web.servlet.result.MockMvcResultMatchers.jsonPath;
import static org.springframework.test.web.servlet.result.MockMvcResultMatchers.status;

@WebMvcTest(PermissionDemoController.class)
@Import({JwtAuthInterceptor.class, PermissionAspect.class, WebMvcConfig.class})
class PermissionDemoControllerTest {

    @Autowired private MockMvc mvc;
    @MockBean private UserServiceClientClient clientClient;
    @MockBean private TokenServerUserClient tokenClient;

    @Test
    void public_endpoint_is_accessible_without_auth() throws Exception {
        mvc.perform(get("/demo/perm/public"))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.code").value("OK"))
                .andExpect(jsonPath("$.data.message").value("public, no auth needed"));
    }

    @Test
    void internal_calls_client_sdk_createRole() throws Exception {
        Role role = new Role();
        role.setId("role-42");
        role.setName("TASK_INTERNAL_ADMIN");
        when(clientClient.createRole(any(Map.class))).thenReturn(role);

        mvc.perform(get("/demo/perm/internal")
                        .header("X-User-Id", "internal-svc")
                        .header("X-Permissions", "TASK_INTERNAL_ADMIN"))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.code").value("OK"))
                .andExpect(jsonPath("$.data.id").value("role-42"))
                .andExpect(jsonPath("$.data.name").value("TASK_INTERNAL_ADMIN"));

        ArgumentCaptor<Map<String, Object>> captor = newCaptor();
        verify(clientClient).createRole(captor.capture());
        assertEquals("TASK_INTERNAL_ADMIN", captor.getValue().get("name"));
    }

    @Test
    void resource_issue_token_works() throws Exception {
        ResourceTokenInfo info = new ResourceTokenInfo();
        info.setResourceToken("rt-xyz");
        info.setExpiresIn(600L);
        when(tokenClient.issueResource(anyString(), anyString(), anyString(), anyLong())).thenReturn(info);

        mvc.perform(get("/demo/perm/resource")
                        .header("X-User-Id", "alice"))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.code").value("OK"))
                .andExpect(jsonPath("$.data.resource_token").value("rt-xyz"))
                .andExpect(jsonPath("$.data.expires_in").value(600));

        verify(tokenClient).issueResource("task", "42", "read", 600L);
    }

    @SuppressWarnings("unchecked")
    private static <T> ArgumentCaptor<T> newCaptor() {
        return (ArgumentCaptor<T>) (ArgumentCaptor<?>) ArgumentCaptor.forClass(Map.class);
    }
}

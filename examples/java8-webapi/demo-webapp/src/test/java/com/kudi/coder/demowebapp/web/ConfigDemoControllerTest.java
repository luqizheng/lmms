package io.digua.web;

import io.digua.ConfigPoller;
import io.digua.WebMvcConfig;
import io.digua.JwtAuthInterceptor;
import io.digua.PermissionAspect;
import org.junit.jupiter.api.Test;
import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.boot.test.autoconfigure.web.servlet.WebMvcTest;
import org.springframework.boot.test.mock.mockito.MockBean;
import org.springframework.context.annotation.Import;
import org.springframework.test.web.servlet.MockMvc;

import static org.mockito.Mockito.when;
import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.get;
import static org.springframework.test.web.servlet.result.MockMvcResultMatchers.jsonPath;
import static org.springframework.test.web.servlet.result.MockMvcResultMatchers.status;

@WebMvcTest(ConfigDemoController.class)
@Import({JwtAuthInterceptor.class, PermissionAspect.class, WebMvcConfig.class})
class ConfigDemoControllerTest {

    @Autowired private MockMvc mvc;
    @MockBean private ConfigPoller configPoller;

    @Test
    void show_returns_cached_value() throws Exception {
        when(configPoller.get("foo")).thenReturn("bar");

        mvc.perform(get("/demo/config/show?key=foo")
                        .header("X-User-Id", "alice"))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.code").value("OK"))
                .andExpect(jsonPath("$.data.value").value("bar"));
    }
}

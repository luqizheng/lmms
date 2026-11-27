package io.digua.demowebapp;

import io.digua.notificationsdk.NotificationServiceClient;
import io.digua.token.TokenServerClientClient;
import io.digua.token.TokenServerUserClient;
import io.digua.user.UserServiceClientClient;
import io.digua.user.UserServiceUserClient;
import org.junit.Test;
import org.junit.runner.RunWith;
import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.boot.test.autoconfigure.web.servlet.AutoConfigureMockMvc;
import org.springframework.boot.test.context.SpringBootTest;
import org.springframework.boot.test.mock.mockito.MockBean;
import org.springframework.test.context.junit4.SpringRunner;
import org.springframework.test.web.servlet.MockMvc;

import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.get;
import static org.springframework.test.web.servlet.result.MockMvcResultMatchers.jsonPath;
import static org.springframework.test.web.servlet.result.MockMvcResultMatchers.status;

@RunWith(SpringRunner.class)
@SpringBootTest
@AutoConfigureMockMvc
public class DemoApplicationSmokeTest {

    @Autowired
    private MockMvc mockMvc;

    @MockBean
    private UserServiceUserClient userServiceUserClient;

    @MockBean
    private UserServiceClientClient userServiceClientClient;

    @MockBean
    private TokenServerUserClient tokenServerUserClient;

    @MockBean
    private TokenServerClientClient tokenServerClientClient;

    @MockBean
    private NotificationServiceClient notificationServiceClient;

    @Test
    public void publicEndpointUsesTheExampleResponseEnvelope() throws Exception {
        mockMvc.perform(get("/demo/perm/public"))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.code").value("OK"))
                .andExpect(jsonPath("$.data.message").value("public, no auth needed"));
    }
}

package com.kudi.coder.demowebapp.config;

import com.kudi.coder.auditsdk.AuditServiceClient;
import com.kudi.coder.httpclient.KudiHttpClient;
import com.kudi.coder.sdk.token.TokenServerUserClient;
import com.kudi.coder.sdk.user.UserServiceClientClient;
import com.kudi.coder.sdk.user.UserServiceUserClient;
import org.slf4j.MDC;
import org.springframework.context.annotation.Bean;
import org.springframework.context.annotation.Configuration;
import org.springframework.web.client.RestTemplate;

import java.util.function.Supplier;

@Configuration
public class SdkConfig {

    @Bean
    public AuditServiceClient auditClient(KudiCoderProperties p) {
        Supplier<String> bearer = () -> null;
        return new AuditServiceClient(p.getGateway().getBaseUrl(), bearer);
    }

    @Bean
    public KudiHttpClient kudiHttpClient() {
        // KudiHttpClient is stateless w.r.t. base URL: each call passes a full URL.
        // Gateway base URL lives in KudiCoderProperties.gateway.baseUrl and is composed
        // by callers (controllers) when issuing requests.
        return KudiHttpClient.create();
    }

    @Bean
    public UserServiceUserClient userServiceUserClient(KudiCoderProperties p) {
        return UserServiceUserClient.login(
                p.getGateway().getBaseUrl(),
                p.getIam().getUsername(),
                p.getIam().getPassword());
    }

    @Bean
    public TokenServerUserClient tokenServerUserClient(KudiCoderProperties p) {
        return TokenServerUserClient.login(
                p.getGateway().getBaseUrl(),
                p.getIam().getUsername(),
                p.getIam().getPassword());
    }

    @Bean
    public UserServiceClientClient userServiceClientClient(KudiCoderProperties p) {
        return UserServiceClientClient.create(
                p.getGateway().getBaseUrl(),
                p.getClient().getApiKey(),
                p.getClient().getApiSecret(),
                p.getClient().getCode());
    }

    @Bean
    public RestTemplate restTemplate() {
        RestTemplate rt = new RestTemplate();
        rt.getInterceptors().add((req, body, ex) -> {
            String rid = MDC.get("requestId");
            if (rid != null) {
                req.getHeaders().add("X-Request-Id", rid);
            }
            return ex.execute(req, body);
        });
        return rt;
    }
}
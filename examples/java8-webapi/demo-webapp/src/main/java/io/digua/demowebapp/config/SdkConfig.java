package io.digua.config;

import io.digua.httpclient.KudiHttpClient;
import io.digua.notificationsdk.NotificationServiceClient;
import io.digua.token.TokenServerClientClient;
import io.digua.token.TokenServerUserClient;
import io.digua.user.UserServiceClientClient;
import io.digua.user.UserServiceUserClient;
import org.springframework.context.annotation.Bean;
import org.springframework.context.annotation.Configuration;

@Configuration
public class SdkConfig {

    @Bean
    public UserServiceUserClient userServiceUserClient(KudiCoderProperties properties) {
        return UserServiceUserClient.login(
                properties.getGateway().getBaseUrl(),
                properties.getIam().getUsername(),
                properties.getIam().getPassword());
    }

    @Bean
    public TokenServerUserClient tokenServerUserClient(KudiCoderProperties properties) {
        return TokenServerUserClient.login(
                properties.getGateway().getBaseUrl(),
                properties.getIam().getUsername(),
                properties.getIam().getPassword());
    }

    @Bean
    public UserServiceClientClient userServiceClientClient(KudiCoderProperties properties) {
        return UserServiceClientClient.create(
                properties.getGateway().getBaseUrl(),
                properties.getClient().getApiKey(),
                properties.getClient().getApiSecret(),
                properties.getClient().getCode());
    }

    @Bean
    public TokenServerClientClient tokenServerClientClient(KudiCoderProperties properties) {
        return TokenServerClientClient.create(
                properties.getGateway().getBaseUrl(),
                properties.getClient().getApiKey(),
                properties.getClient().getApiSecret(),
                properties.getClient().getCode());
    }

    @Bean
    public KudiHttpClient kudiHttpClient() {
        return KudiHttpClient.create();
    }

    @Bean
    public NotificationServiceClient notificationServiceClient(KudiCoderProperties properties) {
        // Supply a current bearer token here when the host application manages one.
        return new NotificationServiceClient(properties.getGateway().getBaseUrl(), () -> null);
    }
}
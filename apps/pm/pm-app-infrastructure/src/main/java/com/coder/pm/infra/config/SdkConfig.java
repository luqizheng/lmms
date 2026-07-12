package com.coder.pm.infra.config;

import com.kudi.coder.auditsdk.AuditServiceClient;
import com.kudi.coder.configsdk.ConfigServiceClient;
import com.kudi.coder.filesdk.FileServiceClient;
import com.kudi.coder.notificationsdk.NotificationServiceClient;
import org.springframework.beans.factory.annotation.Value;
import org.springframework.context.annotation.Bean;
import org.springframework.context.annotation.Configuration;

import java.util.function.Supplier;

/**
 * Wires the four low-memory-micro-services SDK clients as Spring beans so they can be
 * dependency-injected anywhere in apps/pm (audit reporter, file attachment service, etc.).
 *
 * <p>Bearer source: for MVP we use the static token from {@code pm.sdk.bearer.static-token}
 * (default {@code dev-bypass-token}). Future iterations can swap this supplier for one that
 * pulls the caller's JWT out of {@code SecurityContextHolder} so SDK calls forward the
 * originating user's identity to the Rust services.
 *
 * <p>Endpoints come from {@code application.yml} keys {@code pm.sdk.{audit,file,notification,config}.base-url},
 * each defaulting to {@code ${LOWMEM_GATEWAY_BASE:http://localhost:3000}}.
 */
@Configuration
public class SdkConfig {

    @Value("${pm.sdk.audit.base-url:http://localhost:3000}")
    private String auditBaseUrl;

    @Value("${pm.sdk.file.base-url:http://localhost:3000}")
    private String fileBaseUrl;

    @Value("${pm.sdk.notification.base-url:http://localhost:3000}")
    private String notificationBaseUrl;

    @Value("${pm.sdk.config.base-url:http://localhost:3000}")
    private String configBaseUrl;

    @Value("${pm.sdk.bearer.static-token:dev-bypass-token}")
    private String staticBearer;

    private Supplier<String> bearerSupplier() {
        return () -> staticBearer;
    }

    @Bean
    public AuditServiceClient auditServiceClient() {
        return new AuditServiceClient(auditBaseUrl, bearerSupplier());
    }

    @Bean
    public FileServiceClient fileServiceClient() {
        return new FileServiceClient(fileBaseUrl, bearerSupplier());
    }

    @Bean
    public NotificationServiceClient notificationServiceClient() {
        return new NotificationServiceClient(notificationBaseUrl, bearerSupplier());
    }

    @Bean
    public ConfigServiceClient configServiceClient() {
        return new ConfigServiceClient(configBaseUrl, bearerSupplier());
    }
}

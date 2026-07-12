package com.coder.pm;

import org.mybatis.spring.annotation.MapperScan;
import org.springframework.boot.SpringApplication;
import org.springframework.boot.autoconfigure.SpringBootApplication;
import org.springframework.boot.context.properties.ConfigurationPropertiesScan;
import org.springframework.context.annotation.ComponentScan;
import org.springframework.scheduling.annotation.EnableAsync;

/**
 * PM Platform entry point.
 *
 * <p>Wires together:
 * <ul>
 *   <li>{@code pm-app-api} — REST controllers (populated in subsequent PRs)</li>
 *   <li>{@code pm-app-domain} — domain services + repositories</li>
 *   <li>{@code pm-app-infrastructure} — MyBatis-Plus mappers + Flyway + SDK beans</li>
 *   <li>{@code pm-app-llm} — {@code LlmProvider} interface + {@code NoopLlmProvider}</li>
 *   <li>low-memory-micro-services SDK beans (audit / file / notification / config)</li>
 * </ul>
 *
 * <p>Port: {@code 8088}. Context path: {@code /pm-api}.
 */
@SpringBootApplication
@EnableAsync
@ConfigurationPropertiesScan(basePackages = "com.coder.pm")
@MapperScan("com.coder.pm.infra.persistence.mapper")
@ComponentScan(basePackages = {
        "com.coder.pm",
        // SDK beans expose @Component-annotated clients we want Spring to manage.
        "com.kudi.coder.auditsdk",
        "com.kudi.coder.filesdk",
        "com.kudi.coder.notificationsdk",
        "com.kudi.coder.configsdk"
})
public class PmApplication {

    public static void main(String[] args) {
        SpringApplication.run(PmApplication.class, args);
    }
}

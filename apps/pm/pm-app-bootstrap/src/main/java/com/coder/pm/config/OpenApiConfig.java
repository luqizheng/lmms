package com.coder.pm.config;

import io.swagger.v3.oas.models.Components;
import io.swagger.v3.oas.models.OpenAPI;
import io.swagger.v3.oas.models.info.Info;
import io.swagger.v3.oas.models.info.License;
import io.swagger.v3.oas.models.security.SecurityRequirement;
import io.swagger.v3.oas.models.security.SecurityScheme;
import org.springframework.context.annotation.Bean;
import org.springframework.context.annotation.Configuration;

/**
 * springdoc-openapi metadata.
 *
 * <p>Swagger UI: {@code http://localhost:8088/pm-api/swagger-ui/index.html}
 */
@Configuration
public class OpenApiConfig {

    private static final String SECURITY_SCHEME_NAME = "bearerAuth";

    @Bean
    public OpenAPI pmOpenApi() {
        return new OpenAPI()
                .info(new Info()
                        .title("PM Platform API")
                        .description("Requirements + Knowledge Base + LLM-assisted analysis.")
                        .version("0.1.0")
                        .license(new License()
                                .name("Internal — Kudi Coder Platform")
                                .url("https://kudi.repos")))
                .components(new Components()
                        .addSecuritySchemes(SECURITY_SCHEME_NAME,
                                new SecurityScheme()
                                        .type(SecurityScheme.Type.HTTP)
                                        .scheme("bearer")
                                        .bearerFormat("JWT")))
                .addSecurityItem(new SecurityRequirement().addList(SECURITY_SCHEME_NAME));
    }
}

package com.kudi.coder.pmd.config;

import io.swagger.v3.oas.models.OpenAPI;
import io.swagger.v3.oas.models.info.Info;
import io.swagger.v3.oas.models.servers.Server;
import org.springframework.context.annotation.Bean;
import org.springframework.context.annotation.Configuration;

import java.util.List;

@Configuration
public class OpenApiConfig {

    @Bean
    public OpenAPI projectServiceOpenApi() {
        return new OpenAPI()
                .info(new Info()
                        .title("project-service")
                        .version("0.1.0")
                        .description("Software project management domain service (Phase 9 validation MVP)."))
                .servers(List.of(
                        new Server().url("http://localhost:3000/v1/project").description("api-gateway")
                ));
    }
}

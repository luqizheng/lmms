package com.coder.pm.config;

import com.coder.pm.web.JwtAuthFilter;
import org.springframework.context.annotation.Bean;
import org.springframework.context.annotation.Configuration;
import org.springframework.http.HttpMethod;
import org.springframework.security.config.annotation.method.configuration.EnableMethodSecurity;
import org.springframework.security.config.annotation.web.builders.HttpSecurity;
import org.springframework.security.config.annotation.web.configurers.AbstractHttpConfigurer;
import org.springframework.security.config.http.SessionCreationPolicy;
import org.springframework.security.web.SecurityFilterChain;
import org.springframework.security.web.authentication.UsernamePasswordAuthenticationFilter;
import org.springframework.web.cors.CorsConfiguration;
import org.springframework.web.cors.CorsConfigurationSource;
import org.springframework.web.cors.UrlBasedCorsConfigurationSource;

import java.util.List;

/**
 * Security filter chain.
 *
 * <p>Public endpoints (no JWT required):
 * <ul>
 *   <li>{@code /pm-api/v1/health/**} — Kubernetes-style liveness/readiness</li>
 *   <li>{@code /pm-api/v3/api-docs/**} + {@code /pm-api/swagger-ui/**} — OpenAPI</li>
 *   <li>{@code /pm-api/actuator/**} — Spring Boot Actuator</li>
 *   <li>{@code /error}</li>
 *   <li>All {@code OPTIONS} pre-flights (CORS)</li>
 * </ul>
 *
 * <p>Authenticated endpoints: everything else under {@code /pm-api/v1/**}.
 * JWT verification is delegated to {@link JwtAuthFilter}, which delegates to
 * either {@code JwksJwtVerifier} (RS256 via iam-service) or
 * {@code Hs256JwtVerifier} (local dev) depending on {@code pm.jwt.mode}.
 *
 * <p>{@link EnableMethodSecurity} turns on {@code @PreAuthorize} /
 * {@code @Secured} so future controllers can do per-endpoint permission
 * checks against the {@code Principal}'s roles / permissions authorities.
 */
@Configuration
@EnableMethodSecurity
public class SecurityConfig {

    private final JwtAuthFilter jwtAuthFilter;

    public SecurityConfig(JwtAuthFilter jwtAuthFilter) {
        this.jwtAuthFilter = jwtAuthFilter;
    }

    @Bean
    public SecurityFilterChain filterChain(HttpSecurity http) throws Exception {
        http
                .cors(cors -> cors.configurationSource(corsConfigurationSource()))
                .csrf(AbstractHttpConfigurer::disable)
                .formLogin(AbstractHttpConfigurer::disable)
                .httpBasic(AbstractHttpConfigurer::disable)
                .sessionManagement(s -> s.sessionCreationPolicy(SessionCreationPolicy.STATELESS))
                .authorizeHttpRequests(auth -> auth
                        .requestMatchers(HttpMethod.OPTIONS, "/**").permitAll()
                        .requestMatchers(
                                "/v1/health",
                                "/v1/health/**",
                                "/v3/api-docs",
                                "/v3/api-docs/**",
                                "/swagger-ui",
                                "/swagger-ui/**",
                                "/actuator",
                                "/actuator/**",
                                "/error"
                        ).permitAll()
                        .anyRequest().authenticated()
                )
                .addFilterBefore(jwtAuthFilter, UsernamePasswordAuthenticationFilter.class);

        return http.build();
    }

    @Bean
    public CorsConfigurationSource corsConfigurationSource() {
        CorsConfiguration cfg = new CorsConfiguration();
        cfg.setAllowedOrigins(List.of(
                "http://localhost:5173",  // Vite dev
                "http://localhost:3000",  // frontend docker
                "http://localhost:8080"
        ));
        cfg.setAllowedMethods(List.of("GET", "POST", "PUT", "PATCH", "DELETE", "OPTIONS"));
        cfg.setAllowedHeaders(List.of("*"));
        cfg.setAllowCredentials(true);
        cfg.setMaxAge(3600L);

        UrlBasedCorsConfigurationSource source = new UrlBasedCorsConfigurationSource();
        source.registerCorsConfiguration("/v1/**", cfg);
        return source;
    }
}

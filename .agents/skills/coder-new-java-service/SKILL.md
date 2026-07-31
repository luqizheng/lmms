---
name: coder-new-java-service
description: |
  Scaffold a brand-new Java (Spring Boot 3 + JDK 21) microservice following the
  8-item service standard. Use when the user says "new java service", "create a
  spring boot service", "scaffold X java service", or any equivalent intent to
  add a Java backend to the platform. The service is placed under services/
  alongside Rust services and auto-registered in docker-compose + api-gateway.
---

# coder-new-java-service Skill

Scaffold a new Java Spring Boot microservice that satisfies the **8-item service
standard baseline** (ADR-002) on first commit - no exemptions required.

The service uses a multi-module Maven layout:
`{name}-bootstrap` (entry + security + health) / `{name}-api` (controllers) /
`{name}-domain` (exceptions + domain models) / `{name}-infrastructure`
(MyBatis-Plus + Flyway + SDK beans).

## Service Scope

New Java business services go under `services/{name}/` (same directory as Rust
services). The `scripts/check-service-standard.js` already detects Java services
via `pom.xml` and validates the 8-item baseline.

## Variable Conventions

The templates below use these placeholders - replace ALL of them consistently:

| Placeholder | Meaning | Example |
|---|---|---|
| `{name}` | kebab-case service name | `billing-service` |
| `{short}` | short name (strip `-service` suffix) | `billing` |
| `{pkg}` | lowercase package segment (no hyphens) | `billing` |
| `{Prefix}` | PascalCase short name | `Billing` |
| `{prefix}` | DB table prefix (add trailing `_`) | `billing_` |
| `{context_path}` | servlet context-path | `/billing-api` |
| `{port}` | external port (default 8090, increment) | `8090` |
| `{purpose}` | one-sentence description | `Billing and invoicing` |

## Gathering Service Info

Ask the user for:

1. **Service name** (kebab-case, e.g. `billing-service`, `order-service`)
2. **Purpose** (one sentence - what business problem this service solves)
3. **Database** (MySQL / none - drives Flyway setup)
4. **Table prefix** (e.g. `bill_`, `order_` - must be registered in `check-db-naming.js`)
5. **Port** (default: 8090, increment to avoid collisions with existing services)
6. **Initial routes** (optional - empty service is fine; routes added later via `coder-add-java-route`)
7. **SDK dependencies** (optional - which Rust services it calls: audit/file/notification/config)

## File Scaffolding

```
services/{name}/
├── pom.xml                                    # Parent POM (packaging=pom)
├── Dockerfile                                 # 3-stage: SDK build -> app build -> JRE runtime
├── AGENTS.md                                  # Service conventions (8-item)
├── README.md                                  # Quick-start + endpoint list
├── config/
│   └── application.yml                        # Externalized config (docker-compose mount target)
├── {short}-app-bootstrap/
│   ├── pom.xml
│   └── src/main/
│       ├── java/com/coder/{pkg}/
│       │   ├── {Prefix}Application.java       # @SpringBootApplication entry
│       │   ├── config/
│       │   │   ├── OpenApiConfig.java          # springdoc metadata
│       │   │   └── SecurityConfig.java         # Filter chain + CORS
│       │   └── web/
│       │       ├── HealthController.java       # /v1/health + /v1/health/ready
│       │       ├── JwtAuthFilter.java          # Decode JWT payload (trust gateway)
│       │       └── exception/
│       │           └── GlobalExceptionHandler.java
│       └── resources/
│           ├── application.yml                 # Defaults (prod profile)
│           └── application-dev.yml             # Dev overrides
├── {short}-app-api/
│   ├── pom.xml
│   └── src/main/java/com/coder/{pkg}/api/
│       └── .gitkeep
├── {short}-app-domain/
│   ├── pom.xml
│   └── src/main/java/com/coder/{pkg}/web/exception/
│       ├── NotFoundException.java
│       └── ConflictException.java
├── {short}-app-infrastructure/
│   ├── pom.xml
│   └── src/main/
│       ├── java/com/coder/{pkg}/infra/
│       │   ├── security/Principal.java         # JWT claims record
│       │   ├── persistence/entity/.gitkeep
│       │   ├── persistence/mapper/.gitkeep
│       │   └── service/.gitkeep
│       └── resources/
│           └── db/migration/V1__init.sql       # Flyway initial schema
└── tests/                                      # Integration test placeholder
    └── .gitkeep
```

## Parent pom.xml Template

```xml
<?xml version="1.0" encoding="UTF-8"?>
<project xmlns="http://maven.apache.org/POM/4.0.0"
         xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"
         xsi:schemaLocation="http://maven.apache.org/POM/4.0.0 http://maven.apache.org/xsd/maven-4.0.0.xsd">
    <modelVersion>4.0.0</modelVersion>

    <parent>
        <groupId>org.springframework.boot</groupId>
        <artifactId>spring-boot-dependencies</artifactId>
        <version>3.3.5</version>
        <relativePath/>
    </parent>

    <groupId>com.coder.{pkg}</groupId>
    <artifactId>{name}-parent</artifactId>
    <version>0.1.0-SNAPSHOT</version>
    <packaging>pom</packaging>
    <name>{Prefix} Service Parent</name>
    <description>{purpose}</description>

    <modules>
        <module>{short}-app-bootstrap</module>
        <module>{short}-app-api</module>
        <module>{short}-app-domain</module>
        <module>{short}-app-infrastructure</module>
    </modules>

    <properties>
        <java.version>21</java.version>
        <maven.compiler.release>21</maven.compiler.release>
        <project.build.sourceEncoding>UTF-8</project.build.sourceEncoding>
        <mybatis-plus.version>3.5.7</mybatis-plus.version>
        <jjwt.version>0.12.6</jjwt.version>
        <springdoc.version>2.6.0</springdoc.version>
        <digua-sdk.version>0.3.0</digua-sdk.version>
    </properties>

    <dependencyManagement>
        <dependencies>
            <!-- Inter-module -->
            <dependency>
                <groupId>com.coder.{pkg}</groupId>
                <artifactId>{short}-app-api</artifactId>
                <version>${project.version}</version>
            </dependency>
            <dependency>
                <groupId>com.coder.{pkg}</groupId>
                <artifactId>{short}-app-domain</artifactId>
                <version>${project.version}</version>
            </dependency>
            <dependency>
                <groupId>com.coder.{pkg}</groupId>
                <artifactId>{short}-app-infrastructure</artifactId>
                <version>${project.version}</version>
            </dependency>

            <!-- Third-party -->
            <dependency>
                <groupId>com.baomidou</groupId>
                <artifactId>mybatis-plus-spring-boot3-starter</artifactId>
                <version>${mybatis-plus.version}</version>
            </dependency>
            <dependency>
                <groupId>io.jsonwebtoken</groupId>
                <artifactId>jjwt-api</artifactId>
                <version>${jjwt.version}</version>
            </dependency>
            <dependency>
                <groupId>io.jsonwebtoken</groupId>
                <artifactId>jjwt-impl</artifactId>
                <version>${jjwt.version}</version>
                <scope>runtime</scope>
            </dependency>
            <dependency>
                <groupId>io.jsonwebtoken</groupId>
                <artifactId>jjwt-jackson</artifactId>
                <version>${jjwt.version}</version>
                <scope>runtime</scope>
            </dependency>
            <dependency>
                <groupId>org.springdoc</groupId>
                <artifactId>springdoc-openapi-starter-webmvc-ui</artifactId>
                <version>${springdoc.version}</version>
            </dependency>
            <dependency>
                <groupId>org.springdoc</groupId>
                <artifactId>springdoc-openapi-starter-digua_common</artifactId>
                <version>${springdoc.version}</version>
            </dependency>
        </dependencies>
    </dependencyManagement>

    <repositories>
        <repository>
            <id>digua-maven</id>
            <name>Internal Maven Repository (Reposilite)</name>
            <url>http://192.168.2.12:18181/releases</url>
        </repository>
    </repositories>

    <build>
        <pluginManagement>
            <plugins>
                <plugin>
                    <groupId>org.apache.maven.plugins</groupId>
                    <artifactId>maven-compiler-plugin</artifactId>
                    <version>3.13.0</version>
                    <configuration>
                        <release>${java.version}</release>
                        <parameters>true</parameters>
                    </configuration>
                </plugin>
                <plugin>
                    <groupId>org.springframework.boot</groupId>
                    <artifactId>spring-boot-maven-plugin</artifactId>
                    <version>3.3.5</version>
                </plugin>
            </plugins>
        </pluginManagement>
    </build>
</project>
```

## {short}-app-bootstrap/pom.xml

```xml
<?xml version="1.0" encoding="UTF-8"?>
<project xmlns="http://maven.apache.org/POM/4.0.0"
         xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"
         xsi:schemaLocation="http://maven.apache.org/POM/4.0.0 http://maven.apache.org/xsd/maven-4.0.0.xsd">
    <modelVersion>4.0.0</modelVersion>
    <parent>
        <groupId>com.coder.{pkg}</groupId>
        <artifactId>{name}-parent</artifactId>
        <version>0.1.0-SNAPSHOT</version>
        <relativePath>../pom.xml</relativePath>
    </parent>
    <artifactId>{short}-app-bootstrap</artifactId>
    <packaging>jar</packaging>
    <name>{Prefix} Bootstrap</name>

    <dependencies>
        <dependency>
            <groupId>com.coder.{pkg}</groupId>
            <artifactId>{short}-app-api</artifactId>
        </dependency>
        <dependency>
            <groupId>com.coder.{pkg}</groupId>
            <artifactId>{short}-app-domain</artifactId>
        </dependency>
        <dependency>
            <groupId>com.coder.{pkg}</groupId>
            <artifactId>{short}-app-infrastructure</artifactId>
        </dependency>

        <dependency>
            <groupId>org.springframework.boot</groupId>
            <artifactId>spring-boot-starter-web</artifactId>
        </dependency>
        <dependency>
            <groupId>org.springframework.boot</groupId>
            <artifactId>spring-boot-starter-security</artifactId>
        </dependency>
        <dependency>
            <groupId>org.springframework.boot</groupId>
            <artifactId>spring-boot-starter-validation</artifactId>
        </dependency>
        <dependency>
            <groupId>org.springframework.boot</groupId>
            <artifactId>spring-boot-starter-actuator</artifactId>
        </dependency>
        <dependency>
            <groupId>org.springdoc</groupId>
            <artifactId>springdoc-openapi-starter-webmvc-ui</artifactId>
        </dependency>
        <dependency>
            <groupId>io.jsonwebtoken</groupId>
            <artifactId>jjwt-api</artifactId>
        </dependency>
        <dependency>
            <groupId>io.jsonwebtoken</groupId>
            <artifactId>jjwt-impl</artifactId>
        </dependency>
        <dependency>
            <groupId>io.jsonwebtoken</groupId>
            <artifactId>jjwt-jackson</artifactId>
        </dependency>

        <dependency>
            <groupId>org.springframework.boot</groupId>
            <artifactId>spring-boot-starter-test</artifactId>
            <scope>test</scope>
        </dependency>
    </dependencies>

    <build>
        <finalName>{short}-app-bootstrap</finalName>
        <plugins>
            <plugin>
                <groupId>org.springframework.boot</groupId>
                <artifactId>spring-boot-maven-plugin</artifactId>
                <configuration>
                    <mainClass>com.coder.{pkg}.{Prefix}Application</mainClass>
                </configuration>
                <executions>
                    <execution>
                        <goals><goal>repackage</goal></goals>
                    </execution>
                </executions>
            </plugin>
        </plugins>
    </build>
</project>
```

## {Prefix}Application.java

```java
package com.coder.{pkg};

import org.mybatis.spring.annotation.MapperScan;
import org.springframework.boot.SpringApplication;
import org.springframework.boot.autoconfigure.SpringBootApplication;
import org.springframework.context.annotation.ComponentScan;

/**
 * {purpose}
 *
 * <p>Port: {port}. Context path: {context_path}.
 */
@SpringBootApplication
@MapperScan("com.coder.{pkg}.infra.persistence.mapper")
@ComponentScan(basePackages = "com.coder.{pkg}")
public class {Prefix}Application {
    public static void main(String[] args) {
        SpringApplication.run({Prefix}Application.class, args);
    }
}
```

## HealthController.java

```java
package com.coder.{pkg}.web;

import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.http.MediaType;
import org.springframework.http.ResponseEntity;
import org.springframework.jdbc.core.JdbcTemplate;
import org.springframework.web.bind.annotation.GetMapping;
import org.springframework.web.bind.annotation.RequestMapping;
import org.springframework.web.bind.annotation.RestController;

import java.time.Instant;
import java.util.LinkedHashMap;
import java.util.Map;

/**
 * Liveness + readiness. Liveness does NOT touch DB; readiness pings MySQL.
 */
@RestController
@RequestMapping("/v1/health")
public class HealthController {

    private static final Logger log = LoggerFactory.getLogger(HealthController.class);
    private static final String SERVICE = "{name}";
    private static final String VERSION = "0.1.0";

    private final JdbcTemplate jdbcTemplate;

    @Autowired
    public HealthController(JdbcTemplate jdbcTemplate) {
        this.jdbcTemplate = jdbcTemplate;
    }

    @GetMapping(produces = MediaType.APPLICATION_JSON_VALUE)
    public ResponseEntity<Map<String, Object>> liveness() {
        return ResponseEntity.ok(baseBody("UP", "UP", "UP"));
    }

    @GetMapping(value = "/ready", produces = MediaType.APPLICATION_JSON_VALUE)
    public ResponseEntity<Map<String, Object>> readiness() {
        String dbStatus;
        try {
            Integer one = jdbcTemplate.queryForObject("SELECT 1", Integer.class);
            dbStatus = one != null && one == 1 ? "UP" : "DOWN";
        } catch (Exception ex) {
            log.warn("readiness: DB ping failed: {}", ex.getMessage());
            dbStatus = "DOWN";
        }
        Map<String, Object> body = baseBody(dbStatus, dbStatus, "UP");
        return "UP".equals(dbStatus)
                ? ResponseEntity.ok(body)
                : ResponseEntity.status(503).body(body);
    }

    private Map<String, Object> baseBody(String status, String db, String deps) {
        Map<String, Object> body = new LinkedHashMap<>();
        body.put("status", status);
        body.put("db", db);
        body.put("dependencies", deps);
        body.put("service", SERVICE);
        body.put("version", VERSION);
        body.put("timestamp", Instant.now().toString());
        return body;
    }
}
```

## SecurityConfig.java

Public endpoints: `/v1/health/**`, `/v3/api-docs/**`, `/swagger-ui/**`, `/actuator/**`, `/error`.
Everything else requires authentication via `JwtAuthFilter`.

```java
package com.coder.{pkg}.config;

import com.coder.{pkg}.web.JwtAuthFilter;
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
                "http://localhost:5173",
                "http://localhost:3000",
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
```

## JwtAuthFilter.java

Decodes the JWT payload **without signature verification** - trusts that
api-gateway has already verified the RS256 signature and checked the blacklist
(see project `AGENTS.md` §身份认证). Extracts claims into a `Principal` and
injects it into the Spring security context.

```java
package com.coder.{pkg}.web;

import com.coder.{pkg}.infra.security.Principal;
import com.fasterxml.jackson.databind.JsonNode;
import com.fasterxml.jackson.databind.ObjectMapper;
import io.jsonwebtoken.Claims;
import io.jsonwebtoken.Jwts;
import jakarta.servlet.FilterChain;
import jakarta.servlet.ServletException;
import jakarta.servlet.http.HttpServletRequest;
import jakarta.servlet.http.HttpServletResponse;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.springframework.security.authentication.UsernamePasswordAuthenticationToken;
import org.springframework.security.core.authority.SimpleGrantedAuthority;
import org.springframework.security.core.context.SecurityContextHolder;
import org.springframework.stereotype.Component;
import org.springframework.web.filter.OncePerRequestFilter;

import java.io.IOException;
import java.util.Base64;
import java.util.List;

/**
 * Decodes JWT payload (no signature verification - trusts api-gateway).
 * If no Authorization header is present, the security context is left empty
 * and Spring Security will reject the request (401) for protected endpoints.
 */
@Component
public class JwtAuthFilter extends OncePerRequestFilter {

    private static final Logger log = LoggerFactory.getLogger(JwtAuthFilter.class);
    private static final ObjectMapper MAPPER = new ObjectMapper();

    @Override
    protected void doFilterInternal(HttpServletRequest req, HttpServletResponse res, FilterChain chain)
            throws ServletException, IOException {
        String header = req.getHeader("Authorization");
        if (header != null && header.startsWith("Bearer ")) {
            String token = header.substring(7);
            try {
                // Parse without verification - trust gateway
                Claims claims = Jwts.parser()
                        .unsecured()
                        .build()
                        .parseUnsecuredClaims(token)
                        .getPayload();
                Principal principal = Principal.fromClaims(claims);
                List<SimpleGrantedAuthority> authorities = principal.permissions().stream()
                        .map(SimpleGrantedAuthority::new)
                        .toList();
                UsernamePasswordAuthenticationToken auth =
                        new UsernamePasswordAuthenticationToken(principal, null, authorities);
                SecurityContextHolder.getContext().setAuthentication(auth);
            } catch (Exception ex) {
                log.debug("JWT decode failed for {}: {}", req.getRequestURI(), ex.getMessage());
                // Leave context empty -> Spring Security returns 401
            }
        }
        chain.doFilter(req, res);
    }
}
```

## Principal.java (infrastructure module)

```java
package com.coder.{pkg}.infra.security;

import io.jsonwebtoken.Claims;

import java.util.List;

/**
 * Identity extracted from the JWT payload. Mirrors the Rust
 * {@code libs/jwt-ctx::Principal} fields. The api-gateway verifies the
 * signature and populates these claims before forwarding.
 */
public record Principal(
        Long userId,
        String username,
        Long tenantId,
        List<String> roles,
        List<String> permissions,
        String scope
) {
    /** Extracts Principal from JWT claims (no verification - trust gateway). */
    @SuppressWarnings("unchecked")
    public static Principal fromClaims(Claims claims) {
        String sub = claims.getSubject();
        Long userId = null;
        if (sub != null) {
            // sub is a UUID; userId is the first 16 hex chars parsed as long
            String hex = sub.replace("-", "");
            if (hex.length() >= 16) {
                try { userId = Long.parseUnsignedLong(hex.substring(0, 16), 16); }
                catch (NumberFormatException ignored) {}
            }
        }
        Object tid = claims.get("tid");
        Long tenantId = tid instanceof Number n ? n.longValue() : null;
        List<String> roles = claims.get("roles", List.class);
        List<String> perms = claims.get("permissions", List.class);
        if (roles == null) roles = List.of();
        if (perms == null) perms = List.of();
        return new Principal(
                userId,
                claims.get("user_code", String.class),
                tenantId,
                roles,
                perms,
                claims.get("scope", "access")
        );
    }

    /** Convenience for services: actor display name for audit logs. */
    public String displayName() {
        return username != null ? username : "system";
    }
}
```

## OpenApiConfig.java

```java
package com.coder.{pkg}.config;

import io.swagger.v3.oas.models.Components;
import io.swagger.v3.oas.models.OpenAPI;
import io.swagger.v3.oas.models.info.Info;
import io.swagger.v3.oas.models.security.SecurityRequirement;
import io.swagger.v3.oas.models.security.SecurityScheme;
import org.springframework.context.annotation.Bean;
import org.springframework.context.annotation.Configuration;

@Configuration
public class OpenApiConfig {

    private static final String SECURITY_SCHEME_NAME = "bearerAuth";

    @Bean
    public OpenAPI {pkg}OpenApi() {
        return new OpenAPI()
                .info(new Info()
                        .title("{Prefix} API")
                        .description("{purpose}")
                        .version("0.1.0"))
                .components(new Components()
                        .addSecuritySchemes(SECURITY_SCHEME_NAME,
                                new SecurityScheme()
                                        .type(SecurityScheme.Type.HTTP)
                                        .scheme("bearer")
                                        .bearerFormat("JWT")))
                .addSecurityItem(new SecurityRequirement().addList(SECURITY_SCHEME_NAME));
    }
}
```

## GlobalExceptionHandler.java

```java
package com.coder.{pkg}.web.exception;

import com.coder.{pkg}.web.exception.NotFoundException;
import com.coder.{pkg}.web.exception.ConflictException;
import jakarta.servlet.http.HttpServletRequest;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.springframework.http.HttpStatus;
import org.springframework.http.ResponseEntity;
import org.springframework.validation.FieldError;
import org.springframework.web.bind.MethodArgumentNotValidException;
import org.springframework.web.bind.annotation.ExceptionHandler;
import org.springframework.web.bind.annotation.RestControllerAdvice;

import java.time.Instant;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.UUID;

/**
 * Unified error envelope: { code, message, path, timestamp, data }.
 */
@RestControllerAdvice
public class GlobalExceptionHandler {

    private static final Logger log = LoggerFactory.getLogger(GlobalExceptionHandler.class);

    @ExceptionHandler(MethodArgumentNotValidException.class)
    public ResponseEntity<Map<String, Object>> onValidation(MethodArgumentNotValidException ex,
                                                             HttpServletRequest req) {
        List<String> details = ex.getBindingResult().getFieldErrors().stream()
                .map(fe -> fe.getField() + ": " + fe.getDefaultMessage())
                .toList();
        return ResponseEntity.status(HttpStatus.BAD_REQUEST)
                .body(envelope(40001, "validation failed", req, Map.of("errors", details)));
    }

    @ExceptionHandler(IllegalArgumentException.class)
    public ResponseEntity<Map<String, Object>> onIllegalArg(IllegalArgumentException ex,
                                                             HttpServletRequest req) {
        return ResponseEntity.status(HttpStatus.BAD_REQUEST)
                .body(envelope(40000, ex.getMessage() != null ? ex.getMessage() : "bad request", req, null));
    }

    @ExceptionHandler(NotFoundException.class)
    public ResponseEntity<Map<String, Object>> onNotFound(NotFoundException ex, HttpServletRequest req) {
        return ResponseEntity.status(HttpStatus.NOT_FOUND)
                .body(envelope(40401, ex.getMessage(), req, null));
    }

    @ExceptionHandler(ConflictException.class)
    public ResponseEntity<Map<String, Object>> onConflict(ConflictException ex, HttpServletRequest req) {
        return ResponseEntity.status(HttpStatus.CONFLICT)
                .body(envelope(40901, ex.getMessage(), req, null));
    }

    @ExceptionHandler(Exception.class)
    public ResponseEntity<Map<String, Object>> onAny(Exception ex, HttpServletRequest req) {
        String traceId = UUID.randomUUID().toString();
        log.error("unhandled exception traceId={} path={}", traceId, req.getRequestURI(), ex);
        return ResponseEntity.status(HttpStatus.INTERNAL_SERVER_ERROR)
                .body(envelope(50001, "internal server error", req, Map.of("traceId", traceId)));
    }

    private Map<String, Object> envelope(int code, String message, HttpServletRequest req, Object data) {
        Map<String, Object> body = new LinkedHashMap<>();
        body.put("code", code);
        body.put("message", message);
        body.put("path", req.getRequestURI());
        body.put("timestamp", Instant.now().toString());
        body.put("data", data);
        return body;
    }
}
```

## {short}-app-domain/pom.xml + exceptions

```xml
<?xml version="1.0" encoding="UTF-8"?>
<project xmlns="http://maven.apache.org/POM/4.0.0"
         xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"
         xsi:schemaLocation="http://maven.apache.org/POM/4.0.0 http://maven.apache.org/xsd/maven-4.0.0.xsd">
    <modelVersion>4.0.0</modelVersion>
    <parent>
        <groupId>com.coder.{pkg}</groupId>
        <artifactId>{name}-parent</artifactId>
        <version>0.1.0-SNAPSHOT</version>
        <relativePath>../pom.xml</relativePath>
    </parent>
    <artifactId>{short}-app-domain</artifactId>
    <packaging>jar</packaging>
    <name>{Prefix} Domain</name>
    <description>Domain exceptions + business rules (framework-agnostic).</description>
</project>
```

`NotFoundException.java`:
```java
package com.coder.{pkg}.web.exception;

public class NotFoundException extends RuntimeException {
    public NotFoundException(String message) { super(message); }
}
```

`ConflictException.java`:
```java
package com.coder.{pkg}.web.exception;

public class ConflictException extends RuntimeException {
    public ConflictException(String message) { super(message); }
}
```

> **Note**: The exception package is `com.coder.{pkg}.web.exception` (not `.domain.exception`)
> so that `GlobalExceptionHandler` in bootstrap can import them without a circular module
> dependency. This mirrors the `the project-manager repository` convention.

## {short}-app-api/pom.xml

```xml
<?xml version="1.0" encoding="UTF-8"?>
<project xmlns="http://maven.apache.org/POM/4.0.0"
         xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"
         xsi:schemaLocation="http://maven.apache.org/POM/4.0.0 http://maven.apache.org/xsd/maven-4.0.0.xsd">
    <modelVersion>4.0.0</modelVersion>
    <parent>
        <groupId>com.coder.{pkg}</groupId>
        <artifactId>{name}-parent</artifactId>
        <version>0.1.0-SNAPSHOT</version>
        <relativePath>../pom.xml</relativePath>
    </parent>
    <artifactId>{short}-app-api</artifactId>
    <packaging>jar</packaging>
    <name>{Prefix} API</name>
    <description>REST controllers + DTOs.</description>

    <dependencies>
        <dependency>
            <groupId>com.coder.{pkg}</groupId>
            <artifactId>{short}-app-domain</artifactId>
        </dependency>
        <dependency>
            <groupId>com.coder.{pkg}</groupId>
            <artifactId>{short}-app-infrastructure</artifactId>
        </dependency>
        <dependency>
            <groupId>org.springframework.boot</groupId>
            <artifactId>spring-boot-starter-web</artifactId>
        </dependency>
        <dependency>
            <groupId>org.springframework.boot</groupId>
            <artifactId>spring-boot-starter-validation</artifactId>
        </dependency>
        <dependency>
            <groupId>org.springdoc</groupId>
            <artifactId>springdoc-openapi-starter-digua_common</artifactId>
        </dependency>
    </dependencies>
</project>
```

## {short}-app-infrastructure/pom.xml

```xml
<?xml version="1.0" encoding="UTF-8"?>
<project xmlns="http://maven.apache.org/POM/4.0.0"
         xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"
         xsi:schemaLocation="http://maven.apache.org/POM/4.0.0 http://maven.apache.org/xsd/maven-4.0.0.xsd">
    <modelVersion>4.0.0</modelVersion>
    <parent>
        <groupId>com.coder.{pkg}</groupId>
        <artifactId>{name}-parent</artifactId>
        <version>0.1.0-SNAPSHOT</version>
        <relativePath>../pom.xml</relativePath>
    </parent>
    <artifactId>{short}-app-infrastructure</artifactId>
    <packaging>jar</packaging>
    <name>{Prefix} Infrastructure</name>
    <description>MyBatis-Plus + Flyway + SDK integrations.</description>

    <dependencies>
        <dependency>
            <groupId>com.coder.{pkg}</groupId>
            <artifactId>{short}-app-domain</artifactId>
        </dependency>
        <dependency>
            <groupId>com.baomidou</groupId>
            <artifactId>mybatis-plus-spring-boot3-starter</artifactId>
        </dependency>
        <dependency>
            <groupId>org.springframework.boot</groupId>
            <artifactId>spring-boot-starter-jdbc</artifactId>
        </dependency>
        <dependency>
            <groupId>org.flywaydb</groupId>
            <artifactId>flyway-core</artifactId>
        </dependency>
        <dependency>
            <groupId>org.flywaydb</groupId>
            <artifactId>flyway-mysql</artifactId>
        </dependency>
        <dependency>
            <groupId>com.mysql</groupId>
            <artifactId>mysql-connector-j</artifactId>
            <scope>runtime</scope>
        </dependency>
        <dependency>
            <groupId>org.springframework.security</groupId>
            <artifactId>spring-security-core</artifactId>
        </dependency>
        <dependency>
            <groupId>io.jsonwebtoken</groupId>
            <artifactId>jjwt-api</artifactId>
        </dependency>

        <!-- Optional SDK deps - uncomment the ones this service actually calls -->
        <!--
        <dependency>
            <groupId>io.digua</groupId>
            <artifactId>token-server-client-sdk</artifactId>
            <version>${digua-sdk.version}</version>
        </dependency>
        <dependency>
            <groupId>io.digua</groupId>
            <artifactId>token-server-user-sdk</artifactId>
            <version>${digua-sdk.version}</version>
        </dependency>
        <dependency>
            <groupId>io.digua</groupId>
            <artifactId>user-service-user-sdk</artifactId>
            <version>${digua-sdk.version}</version>
        </dependency>
        <dependency>
            <groupId>io.digua</groupId>
            <artifactId>user-service-client-sdk</artifactId>
            <version>${digua-sdk.version}</version>
        </dependency>
        <dependency>
            <groupId>io.digua</groupId>
            <artifactId>audit-service-sdk</artifactId>
            <version>${digua-sdk.version}</version>
        </dependency>
        <dependency>
            <groupId>io.digua</groupId>
            <artifactId>file-service-sdk</artifactId>
            <version>${digua-sdk.version}</version>
        </dependency>
        <dependency>
            <groupId>io.digua</groupId>
            <artifactId>notification-service-sdk</artifactId>
            <version>${digua-sdk.version}</version>
        </dependency>
        <dependency>
            <groupId>io.digua</groupId>
            <artifactId>config-service-sdk</artifactId>
            <version>${digua-sdk.version}</version>
        </dependency>
        -->

        <dependency>
            <groupId>org.junit.jupiter</groupId>
            <artifactId>junit-jupiter</artifactId>
            <version>5.10.2</version>
            <scope>test</scope>
        </dependency>
        <dependency>
            <groupId>org.mockito</groupId>
            <artifactId>mockito-core</artifactId>
            <scope>test</scope>
        </dependency>
        <dependency>
            <groupId>org.mockito</groupId>
            <artifactId>mockito-junit-jupiter</artifactId>
            <scope>test</scope>
        </dependency>
    </dependencies>
</project>
```

## application.yml (bootstrap src/main/resources)

```yaml
spring:
  application:
    name: {name}
  profiles:
    active: ${SPRING_PROFILES_ACTIVE:dev}
  jackson:
    serialization:
      write-dates-as-timestamps: false
    default-property-inclusion: non_null
  datasource:
    url: ${SPRING_DATASOURCE_URL:jdbc:mysql://localhost:3306/superpowers?useSSL=false&serverTimezone=UTC&allowPublicKeyRetrieval=true}
    username: ${SPRING_DATASOURCE_USERNAME:root}
    password: ${SPRING_DATASOURCE_PASSWORD:rootpass}
    driver-class-name: com.mysql.cj.jdbc.Driver
    hikari:
      maximum-pool-size: 10
      minimum-idle: 2
      pool-name: {short}-pool
  flyway:
    enabled: true
    baseline-on-migrate: true
    baseline-version: 0
    locations: classpath:db/migration
    table: {prefix}flyway_history

server:
  port: ${SERVER_PORT:{port}}
  servlet:
    context-path: {context_path}
  forward-headers-strategy: framework
  error:
    include-message: always
    include-stacktrace: never
    include-binding-errors: never

management:
  endpoints:
    web:
      exposure:
        include: health,info,prometheus
  endpoint:
    health:
      show-details: when-authorized
      probes:
        enabled: true

springdoc:
  api-docs:
    path: /v3/api-docs
  swagger-ui:
    path: /swagger-ui/index.html
    operations-sorter: alpha
    tags-sorter: alpha

logging:
  level:
    root: INFO
    com.coder.{pkg}: INFO
    org.springframework.security: WARN
    org.flywaydb: INFO
```

## application-dev.yml

```yaml
# dev profile overrides - dev profile permits requests without a real JWT.
# The JwtAuthFilter will still try to decode the Authorization header if present,
# but for endpoints that are permitAll (health/swagger), no token is needed.

spring:
  datasource:
    url: jdbc:mysql://localhost:3306/superpowers?useSSL=false&serverTimezone=UTC&allowPublicKeyRetrieval=true
    username: root
    password: rootpass

logging:
  level:
    com.coder.{pkg}: DEBUG
    org.springframework.web: INFO
```

## Dockerfile

3-stage build: (1) build Kudi Java SDK into local m2, (2) build the service,
(3) slim JRE runtime. Mirrors `Dockerfile` structure.

```dockerfile
# syntax=docker/dockerfile:1.7
# Multi-stage build for {name} (Spring Boot 3.x, JDK 21).

# ---------------------------------------------------------------------------
# Stage 1: build local Kudi Java SDK into local m2
# ---------------------------------------------------------------------------
FROM maven:3.9.9-eclipse-temurin-21 AS sdk-builder
WORKDIR /build

COPY sdk/java/pom.xml sdk/java/pom.xml
COPY sdk/java/digua_common/pom.xml                 sdk/java/digua_common/pom.xml
COPY sdk/java/digua-http-client/pom.xml      sdk/java/digua-http-client/pom.xml
COPY sdk/java/user-service-user-sdk/pom.xml sdk/java/user-service-user-sdk/pom.xml
COPY sdk/java/user-service-client-sdk/pom.xml sdk/java/user-service-client-sdk/pom.xml
COPY sdk/java/token-server-user-sdk/pom.xml sdk/java/token-server-user-sdk/pom.xml
COPY sdk/java/audit-service-sdk/pom.xml     sdk/java/audit-service-sdk/pom.xml
COPY sdk/java/file-service-sdk/pom.xml      sdk/java/file-service-sdk/pom.xml
COPY sdk/java/notification-service-sdk/pom.xml sdk/java/notification-service-sdk/pom.xml
COPY sdk/java/config-service-sdk/pom.xml    sdk/java/config-service-sdk/pom.xml

COPY sdk/java/digua_common                 sdk/java/digua_common
COPY sdk/java/digua-http-client      sdk/java/digua-http-client
COPY sdk/java/user-service-user-sdk sdk/java/user-service-user-sdk
COPY sdk/java/user-service-client-sdk sdk/java/user-service-client-sdk
COPY sdk/java/token-server-user-sdk sdk/java/token-server-user-sdk
COPY sdk/java/audit-service-sdk     sdk/java/audit-service-sdk
COPY sdk/java/file-service-sdk      sdk/java/file-service-sdk
COPY sdk/java/notification-service-sdk sdk/java/notification-service-sdk
COPY sdk/java/config-service-sdk    sdk/java/config-service-sdk

# token-server-client-sdk 已发布到内网 Reposilite 仓库，通过 Maven 依赖拉取，无需本地构建
# 详见父 pom.xml 的 repositories 配置

RUN --mount=type=cache,id=m2,target=/root/.m2 \
    mvn -f sdk/java/pom.xml -Pjava21 -DskipTests install \
    && mkdir -p /m2-out \
    && cp -a /root/.m2/. /m2-out/

# ---------------------------------------------------------------------------
# Stage 2: build the service
# ---------------------------------------------------------------------------
FROM maven:3.9.9-eclipse-temurin-21 AS builder
WORKDIR /build

COPY --from=sdk-builder /m2-out /root/.m2

COPY services/{name}/pom.xml                              services/{name}/pom.xml
COPY services/{name}/{short}-app-bootstrap/pom.xml        services/{name}/{short}-app-bootstrap/pom.xml
COPY services/{name}/{short}-app-api/pom.xml              services/{name}/{short}-app-api/pom.xml
COPY services/{name}/{short}-app-domain/pom.xml           services/{name}/{short}-app-domain/pom.xml
COPY services/{name}/{short}-app-infrastructure/pom.xml   services/{name}/{short}-app-infrastructure/pom.xml

RUN --mount=type=cache,id=m2,target=/root/.m2 \
    mvn -f services/{name}/pom.xml -Pjava21 dependency:go-offline \
        -DexcludeArtifactIds={short}-app-bootstrap,{short}-app-api,{short}-app-domain,{short}-app-infrastructure || true

COPY services/{name}/{short}-app-bootstrap/src services/{name}/{short}-app-bootstrap/src
COPY services/{name}/{short}-app-api/src       services/{name}/{short}-app-api/src
COPY services/{name}/{short}-app-domain/src    services/{name}/{short}-app-domain/src
COPY services/{name}/{short}-app-infrastructure/src services/{name}/{short}-app-infrastructure/src

RUN --mount=type=cache,id=m2,target=/root/.m2 \
    mvn -f services/{name}/pom.xml -Pjava21 -pl {short}-app-bootstrap -am clean package -DskipTests

# ---------------------------------------------------------------------------
# Stage 3: runtime (Alpine JRE 21)
# ---------------------------------------------------------------------------
FROM eclipse-temurin:21-jre-alpine
RUN apk add --no-cache tzdata curl
WORKDIR /app
COPY --from=builder /build/services/{name}/{short}-app-bootstrap/target/{short}-app-bootstrap.jar app.jar

EXPOSE {port}
ENV JAVA_OPTS="-XX:+UseContainerSupport -XX:MaxRAMPercentage=75.0"

HEALTHCHECK --interval=30s --timeout=5s --start-period=45s --retries=3 \
    CMD curl -fs http://localhost:{port}{context_path}/v1/health || exit 1

ENTRYPOINT ["sh", "-c", "exec java $JAVA_OPTS -jar app.jar --spring.profiles.active=${SPRING_PROFILES_ACTIVE:-dev}"]
```

> **Important**: The Dockerfile `COPY` paths assume the build context is the
> repo root (`context: ..` in docker-compose), same as Rust services. This
> ensures `sdk/java/` is available for the SDK build stage.

## Flyway V1 migration (infrastructure src/main/resources/db/migration/V1__init.sql)

If the service uses MySQL, create an initial migration. Table names MUST use
the service prefix (`{prefix}`). See `contracts/db-conventions.md`.

> **User reference columns** (`user_id` / `created_by` / `updated_by` /
> `operator_id` / `actor_id` / `owner_id`) MUST be `VARCHAR(64)` — the platform
> user id is a random string, not an auto-increment integer. Never `BIGINT`.
> See `contracts/db-conventions.md` §「用户引用字段类型约定」.

```sql
-- V1__init.sql - initial schema for {name}.
-- Table prefix `{prefix}` registered in scripts/check-db-naming.js.

SET NAMES utf8mb4;

CREATE TABLE {prefix}items (
    id BIGINT NOT NULL AUTO_INCREMENT,
    name VARCHAR(128) NOT NULL,
    description TEXT,
    status VARCHAR(32) NOT NULL DEFAULT 'active',
    created_by VARCHAR(64) NOT NULL,
    deleted_at DATETIME DEFAULT NULL,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    PRIMARY KEY (id),
    UNIQUE KEY uk_{prefix}items_name (name),
    KEY idx_{prefix}items_status (status, deleted_at)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci COMMENT='{name} items';
```

## AGENTS.md Template

Copy from `services/agents-template.md` and customize. Key sections:

```markdown
# {name} - Service Conventions

## 8-item service standard

- [x] Dockerfile - `services/{name}/Dockerfile` (multi-stage: maven:3.9.9-eclipse-temurin-21 build + eclipse-temurin:21-jre-alpine runtime)
- [x] config/ - `services/{name}/config/application.yml` + `{short}-app-bootstrap/src/main/resources/{application.yml, application-dev.yml}`
- [x] README.md - `services/{name}/README.md`
- [x] AGENTS.md - this file
- [x] Health endpoint - `{context_path}/v1/health` (liveness) + `{context_path}/v1/health/ready` (readiness w/ DB ping)
- [x] Database migration - Flyway `{short}-app-infrastructure/src/main/resources/db/migration/V1__init.sql`
- [x] Swagger/OpenAPI - springdoc-openapi 2.6.0, `{context_path}/v3/api-docs` + `{context_path}/swagger-ui/index.html`
- [x] Tests - JUnit 5 + Mockito; integration tests under `tests/`

## Configuration

- `SPRING_PROFILES_ACTIVE`: dev / prod
- `SERVER_PORT`: {port}
- `SPRING_DATASOURCE_URL/USER/PASSWORD`: MySQL connection (shared `superpowers` DB)
- `SWAGGER_ENABLED`: not used (Java services always expose swagger-ui; gated by SecurityConfig)

## Endpoint domain

- `{context_path}/v1/health` - liveness
- `{context_path}/v1/health/ready` - readiness
- `{context_path}/v3/api-docs` - OpenAPI JSON
- `{context_path}/swagger-ui/index.html` - Swagger UI

## Table prefix

`{prefix}` - registered in `scripts/check-db-naming.js` whitelist.

## Exemptions

- None (8/8 baseline met on scaffold)
```

## README.md Template

```markdown
# {Prefix} Service

{purpose}

## Quick start

### Local (JDK 21 + Maven 3.9+)

\`\`\`bash
cd services/{name}
mvn -Pjava21 clean package -DskipTests
java -jar {short}-app-bootstrap/target/{short}-app-bootstrap.jar --spring.profiles.active=dev
\`\`\`

### Docker

\`\`\`bash
docker build -t {name}:0.1.0 -f services/{name}/Dockerfile .
docker run --rm -p {port}:{port} \
  -e SPRING_DATASOURCE_URL='jdbc:mysql://host.docker.internal:3306/superpowers?useSSL=false&serverTimezone=UTC&allowPublicKeyRetrieval=true' \
  -e SPRING_PROFILES_ACTIVE=dev \
  {name}:0.1.0
\`\`\`

### Verify

\`\`\`bash
curl http://localhost:{port}{context_path}/v1/health
curl http://localhost:{port}{context_path}/v3/api-docs
\`\`\`

## Endpoints

| Method | Path | Description |
|---|---|---|
| GET | `{context_path}/v1/health` | Liveness |
| GET | `{context_path}/v1/health/ready` | Readiness (DB ping) |
| GET | `{context_path}/v3/api-docs` | OpenAPI JSON |
| GET | `{context_path}/swagger-ui/index.html` | Swagger UI |
```

## Auto-Registration (3 append operations)

After creating all files, the skill MUST perform these 3 registrations so the
service is reachable. Do not leave the user to do them manually.

### 1. Append to `infrastructure/docker-compose-services.yml`

Insert before the `volumes:` section (or before the `# >>> business-services`
anchor if it exists):

```yaml
  # ─────────── {name} (Java + Spring Boot) ───────────
  {name}:
    build:
      context: ..
      dockerfile: services/{name}/Dockerfile
    ports:
      - "${{name-upper}_PORT_EXPORT:-{port}}:{port}"
    labels:
      - "SERVICE_NAME={name}"
      - "SERVICE_ID={name}"
      - "SERVICE_PORT={port}"
      - "SERVICE_TAGS=backend,{name},java"
      - "SERVICE_{port}_CHECK_HTTP={context_path}/v1/health"
      - "SERVICE_{port}_CHECK_INTERVAL=15s"
      - "SERVICE_{port}_CHECK_TIMEOUT=3s"
    environment:
      TENANT_ID: "${TENANT_ID}"
      SPRING_DATASOURCE_URL: "${DATABASE_URL_JDBC}"
      SPRING_DATASOURCE_USERNAME: "${MYSQL_USER}"
      SPRING_DATASOURCE_PASSWORD: "${MYSQL_PASSWORD}"
      SPRING_PROFILES_ACTIVE: "${SPRING_PROFILES_ACTIVE:-dev}"
      OTEL_EXPORTER_OTLP_ENDPOINT: "http://otel-collector:4317"
      JAVA_OPTS: "-XX:+UseG1GC -XX:MaxRAMPercentage=75.0"
    depends_on:
      mysql:
        condition: service_healthy
      consul:
        condition: service_healthy
    networks:
      - superpowers
```

> `{name-upper}` = `{name}` in uppercase with hyphens replaced by underscores
> (e.g. `billing-service` -> `BILLING_SERVICE`).
> `DATABASE_URL_JDBC` should be added to `infrastructure/.env` if not already
> present (format: `jdbc:mysql://mysql:3306/superpowers?useSSL=false&...`).

### 2. Append to `infrastructure/config/api-gateway/routes/{short}.router.toml`

Create this file so api-gateway forwards `/v1/{short}/*` to the new service:

```toml
[[routes]]
upstream = "/v1/{short}/*"
downstream = "http://{name}:{port}{context_path}/v1/{short}/*"
service = "{name}"
public = false
ws = false
```

> The gateway strips `/v1/{short}` prefix and forwards to
> `{context_path}/v1/{short}`. The service's controllers should define routes
> under `/v1/{short}/...` (relative to `context-path`).
> Verify: `cargo run -p api-gateway -- validate --routes infrastructure/config/api-gateway/routes`

### 3. Add table prefix to `scripts/check-db-naming.js`

Find the `SERVICE_PREFIXES` or prefix whitelist in the script and add:

```javascript
  '{name}': '{prefix}',
```

If the script uses a different structure (e.g. an array of objects), follow the
existing pattern. Run `node scripts/check-db-naming.js` to verify.

### 4. (Optional) Add to `infrastructure/.env`

```
{name-upper}_PORT_EXPORT={port}
```

## Validation (run after scaffolding)

```bash
# 1. Service standard gate (must pass 8/8 for Java)
node scripts/check-service-standard.js
# Expected: ✓ 8/8 items for {name}

# 2. DB naming check
node scripts/check-db-naming.js

# 3. Spec check (OpenAPI / error codes / permissions)
node scripts/check-spec.js

# 4. Build (requires JDK 21 + Maven 3.9+)
cd services/{name}
mvn -Pjava21 clean package -DskipTests

# 5. Gateway route validation
cargo run -p api-gateway -- validate --routes infrastructure/config/api-gateway/routes

# 6. Docker build (from repo root)
docker build -t {name}:0.1.0 -f services/{name}/Dockerfile .
```

## Common Mistakes to Avoid

- ❌ Forgetting to register the table prefix in `check-db-naming.js` -> CI fails on the first migration
- ❌ Using a port that collides with an existing service (check `infrastructure/README.md` port table)
- ❌ Putting controllers in the wrong module (they go in `{short}-app-api`, not bootstrap)
- ❌ Forgetting `@MapperScan` in the Application class -> MyBatis-Plus mappers not discovered
- ❌ Not copying `services/agents-template.md` as the AGENTS.md base -> CI service-standard check fails
- ❌ Hardcoding MySQL URL in `application.yml` instead of using `${SPRING_DATASOURCE_URL}` env var
- ❌ Forgetting the `context-path: {context_path}` in `application.yml` -> health check path mismatch in Dockerfile HEALTHCHECK
- ❌ Dockerfile build context not set to repo root (`context: ..`) -> SDK stage can't find `sdk/java/`

## Out of Scope

- JWKS signature verification (this template trusts the api-gateway; for end-to-end verification like `the project-manager repository`, implement a `JwksJwtVerifier` separately)
- LLM provider integration (that's `the project-manager repository` specific; add per-service if needed)
- Frontend integration (separate workflow)
- Production deployment configs (handled at infra layer)

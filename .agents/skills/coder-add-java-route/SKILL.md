---
name: coder-add-java-route
description: |
  Scaffold a new REST endpoint (Controller + DTO + optional entity/mapper/service)
  in an existing Java Spring Boot service with springdoc OpenAPI annotations.
  Use when the user says "add a route", "add an endpoint", "add a controller",
  "add an API" for a Java service, or any equivalent intent.
---

# coder-add-java-route Skill

Scaffold a new REST route in an existing Java Spring Boot microservice.

## Service Scope

Any Java service under `services/{name}/` (detected by `pom.xml`). The service
must have been scaffolded by `coder-new-java-service` or follow the same
multi-module Maven layout (`{short}-app-bootstrap` / `{short}-app-api` /
`{short}-app-domain` / `{short}-app-infrastructure`).

## Variable Conventions

| Placeholder | Meaning | Example |
|---|---|---|
| `{name}` | kebab-case service name | `billing-service` |
| `{short}` | short name (strip `-service`) | `billing` |
| `{pkg}` | package segment | `billing` |
| `{Prefix}` | PascalCase short name | `Billing` |
| `{context_path}` | servlet context-path | `/billing-api` |
| `{feature}` | feature/resource name (kebab-case in URL, lowercase in code) | `invoice` |
| `{Feature}` | PascalCase feature name | `Invoice` |
| `{method}` | HTTP method (lowercase) | `get`, `post`, `put`, `delete` |
| `{Method}` | HTTP method (uppercase) | `GET`, `POST`, `PUT`, `DELETE` |

## Gathering Route Info

Ask the user for (skip any the user already provided):

1. **Service name** (e.g. `billing-service`)
2. **Feature/resource name** (e.g. `invoice`, `order` - used for package, class, and URL path)
3. **HTTP method** (`GET`, `POST`, `PUT`, `PATCH`, `DELETE`)
4. **Route path** (e.g. `/v1/invoices` or `/v1/invoices/{id}/items` - relative to `context-path`)
5. **Request DTO** (fields + types, or "none" for GET/DELETE)
6. **Response DTO** (fields + types, or use the entity directly)
7. **Persistence needed?** (yes -> create entity + mapper + service; no -> create a stub service)
8. **OpenAPI tag** (grouping label, defaults to `{feature}s`)

## File Scaffolding

### Step 1: Create the Controller

File: `services/{name}/{short}-app-api/src/main/java/com/coder/{pkg}/api/{feature}/{Feature}Controller.java`

```java
package com.coder.{pkg}.api.{feature};

import com.coder.{pkg}.infra.persistence.entity.{Feature};
import com.coder.{pkg}.infra.service.{Feature}Service;
import io.swagger.v3.oas.annotations.Operation;
import io.swagger.v3.oas.annotations.tags.Tag;
import jakarta.validation.Valid;
import jakarta.validation.constraints.NotBlank;
import org.springframework.http.ResponseEntity;
import org.springframework.web.bind.annotation.*;

import java.util.Map;

/**
 * REST endpoints for {feature} lifecycle.
 *
 * <pre>
 * {Method} {context_path}/v1/{feature}s  {action}
 * </pre>
 */
@RestController
@RequestMapping("/v1/{feature}s")
@Tag(name = "{feature}s", description = "{Feature} management")
public class {Feature}Controller {

    private final {Feature}Service service;

    public {Feature}Controller({Feature}Service service) {
        this.service = service;
    }

    @PostMapping
    @Operation(summary = "Create a {feature}")
    public ResponseEntity<{Feature}> create(@RequestBody @Valid Create{Feature}Request req) {
        {Feature} draft = new {Feature}();
        draft.setName(req.name());
        draft.setDescription(req.description());
        {Feature} saved = service.create(draft);
        return ResponseEntity.status(201).body(saved);
    }

    @GetMapping
    @Operation(summary = "Paged {feature} list")
    public Map<String, Object> list(@RequestParam(defaultValue = "1") int page,
                                    @RequestParam(defaultValue = "20") int size,
                                    @RequestParam(required = false) String status) {
        var p = service.page(page, size, status);
        return Map.of(
                "items", p.getRecords(),
                "total", p.getTotal(),
                "page", p.getCurrent(),
                "size", p.getSize()
        );
    }

    @GetMapping("/{id}")
    @Operation(summary = "{Feature} detail")
    public {Feature} detail(@PathVariable long id) {
        return service.findActiveOrThrow(id);
    }

    @PatchMapping("/{id}")
    @Operation(summary = "Patch {feature} fields")
    public {Feature} patch(@PathVariable long id, @RequestBody Update{Feature}Request req) {
        {Feature} patch = new {Feature}();
        patch.setName(req.name());
        patch.setDescription(req.description());
        return service.update(id, patch);
    }

    @DeleteMapping("/{id}")
    @Operation(summary = "Archive (soft-delete) {feature}")
    public ResponseEntity<Void> archive(@PathVariable long id) {
        service.archive(id);
        return ResponseEntity.noContent().build();
    }

    // --- DTOs (records) ---

    public record Create{Feature}Request(
            @NotBlank String name,
            String description
    ) {}

    public record Update{Feature}Request(
            String name,
            String description
    ) {}
}
```

Rules:
- Controllers go in `{short}-app-api` module, package `com.coder.{pkg}.api.{feature}`.
- DTOs are `record` types declared inside the controller (or in a separate `dto` subpackage if they grow large).
- Use `@Valid` + `jakarta.validation.constraints` for request validation.
- `@Tag` groups endpoints in Swagger UI; `@Operation` provides the summary.
- springdoc **auto-discovers** `@RestController` classes - no manual OpenAPI registration needed (unlike Rust's utoipa).
- For path params use `@PathVariable long id`; for query params use `@RequestParam`.

### Step 2: Create the Entity (if persistence needed)

File: `services/{name}/{short}-app-infrastructure/src/main/java/com/coder/{pkg}/infra/persistence/entity/{Feature}.java`

```java
package com.coder.{pkg}.infra.persistence.entity;

import com.baomidou.mybatisplus.annotation.IdType;
import com.baomidou.mybatisplus.annotation.TableField;
import com.baomidou.mybatisplus.annotation.TableId;
import com.baomidou.mybatisplus.annotation.TableLogic;
import com.baomidou.mybatisplus.annotation.TableName;

import java.time.LocalDateTime;

/**
 * Maps to {prefix}{feature}s table.
 */
@TableName("{prefix}{feature}s")
public class {Feature} {

    @TableId(type = IdType.AUTO)
    private Long id;

    @TableField("name")
    private String name;

    @TableField("description")
    private String description;

    @TableField("status")
    private String status;

    @TableField("created_by")
    private Long createdBy;

    @TableLogic(value = "NULL", delval = "NOW()")
    @TableField("deleted_at")
    private LocalDateTime deletedAt;

    @TableField("created_at")
    private LocalDateTime createdAt;

    @TableField("updated_at")
    private LocalDateTime updatedAt;

    public Long getId() { return id; }
    public void setId(Long id) { this.id = id; }

    public String getName() { return name; }
    public void setName(String name) { this.name = name; }

    public String getDescription() { return description; }
    public void setDescription(String description) { this.description = description; }

    public String getStatus() { return status; }
    public void setStatus(String status) { this.status = status; }

    public Long getCreatedBy() { return createdBy; }
    public void setCreatedBy(Long createdBy) { this.createdBy = createdBy; }

    public LocalDateTime getDeletedAt() { return deletedAt; }
    public void setDeletedAt(LocalDateTime deletedAt) { this.deletedAt = deletedAt; }

    public LocalDateTime getCreatedAt() { return createdAt; }
    public void setCreatedAt(LocalDateTime createdAt) { this.createdAt = createdAt; }

    public LocalDateTime getUpdatedAt() { return updatedAt; }
    public void setUpdatedAt(LocalDateTime updatedAt) { this.updatedAt = updatedAt; }
}
```

### Step 3: Create the Mapper

File: `services/{name}/{short}-app-infrastructure/src/main/java/com/coder/{pkg}/infra/persistence/mapper/{Feature}Mapper.java`

```java
package com.coder.{pkg}.infra.persistence.mapper;

import com.baomidou.mybatisplus.core.mapper.BaseMapper;
import com.coder.{pkg}.infra.persistence.entity.{Feature};
import org.apache.ibatis.annotations.Mapper;

@Mapper
public interface {Feature}Mapper extends BaseMapper<{Feature}> {
}
```

### Step 4: Create the Service

File: `services/{name}/{short}-app-infrastructure/src/main/java/com/coder/{pkg}/infra/service/{Feature}Service.java`

```java
package com.coder.{pkg}.infra.service;

import com.baomidou.mybatisplus.core.conditions.query.QueryWrapper;
import com.baomidou.mybatisplus.core.metadata.IPage;
import com.baomidou.mybatisplus.extension.plugins.pagination.Page;
import com.baomidou.mybatisplus.extension.service.impl.ServiceImpl;
import com.coder.{pkg}.infra.persistence.entity.{Feature};
import com.coder.{pkg}.infra.persistence.mapper.{Feature}Mapper;
import com.coder.{pkg}.web.exception.ConflictException;
import com.coder.{pkg}.web.exception.NotFoundException;
import org.springframework.stereotype.Service;
import org.springframework.transaction.annotation.Transactional;

/**
 * {Feature} domain service. Wraps MyBatis-Plus CRUD with domain rules:
 * <ul>
 *   <li>{@code name} uniqueness (409 if duplicate)</li>
 *   <li>archival is a soft-delete via {@code deleted_at}, not a hard delete</li>
 * </ul>
 */
@Service
public class {Feature}Service extends ServiceImpl<{Feature}Mapper, {Feature}> {

    @Transactional
    public {Feature} create({Feature} draft) {
        Long existing = lambdaQuery()
                .eq({Feature}::getName, draft.getName())
                .count();
        if (existing != null && existing > 0) {
            throw new ConflictException("{feature} name already exists: " + draft.getName());
        }
        if (draft.getStatus() == null) {
            draft.setStatus("active");
        }
        save(draft);
        return draft;
    }

    @Transactional
    public {Feature} update(long id, {Feature} patch) {
        {Feature} existing = getById(id);
        if (existing == null) {
            throw new NotFoundException("{feature} not found: " + id);
        }
        if (patch.getName() != null) existing.setName(patch.getName());
        if (patch.getDescription() != null) existing.setDescription(patch.getDescription());
        updateById(existing);
        return existing;
    }

    @Transactional
    public void archive(long id) {
        {Feature} entity = getById(id);
        if (entity == null) {
            throw new NotFoundException("{feature} not found: " + id);
        }
        entity.setStatus("archived");
        updateById(entity);
    }

    public {Feature} findActiveOrThrow(long id) {
        {Feature} entity = getById(id);
        if (entity == null || entity.getDeletedAt() != null) {
            throw new NotFoundException("{feature} not found: " + id);
        }
        return entity;
    }

    public IPage<{Feature}> page(int page, int size, String status) {
        QueryWrapper<{Feature}> q = new QueryWrapper<>();
        q.isNull("deleted_at");
        if (status != null && !status.isBlank()) {
            q.eq("status", status);
        }
        q.orderByDesc("created_at");
        return page(new Page<>(page, size), q);
    }
}
```

### Step 5: Create a unit test stub

File: `services/{name}/{short}-app-bootstrap/src/test/java/com/coder/{pkg}/api/{feature}/{Feature}ControllerTest.java`

```java
package com.coder.{pkg}.api.{feature};

import com.coder.{pkg}.infra.persistence.entity.{Feature};
import com.coder.{pkg}.infra.service.{Feature}Service;
import com.fasterxml.jackson.databind.ObjectMapper;
import org.junit.jupiter.api.Test;
import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.boot.test.autoconfigure.web.servlet.WebMvcTest;
import org.springframework.boot.test.mock.bean.MockBean;
import org.springframework.http.MediaType;
import org.springframework.test.web.servlet.MockMvc;

import static org.mockito.ArgumentMatchers.any;
import static org.mockito.Mockito.when;
import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.post;
import static org.springframework.test.web.servlet.result.MockMvcResultMatchers.status;

@WebMvcTest({Feature}Controller.class)
class {Feature}ControllerTest {

    @Autowired
    private MockMvc mockMvc;

    @MockBean
    private {Feature}Service service;

    private final ObjectMapper mapper = new ObjectMapper();

    @Test
    void create_returns_201() throws Exception {
        when(service.create(any({Feature}.class))).thenAnswer(inv -> inv.getArgument(0));

        mockMvc.perform(post("/v1/{feature}s")
                        .contentType(MediaType.APPLICATION_JSON)
                        .content(mapper.writeValueAsString(
                                new {Feature}Controller.Create{Feature}Request("test", "desc"))))
                .andExpect(status().isCreated());
    }
}
```

> **Note**: `@WebMvcTest` loads only the web layer. The `@MockBean` for
> `{Feature}Service` prevents Spring from trying to wire the full DB stack.
> Adjust imports if your Spring Boot version uses `org.springframework.boot.test.mock.mockito.MockBean`
> (3.4+) instead of `org.springframework.boot.test.mock.bean.MockBean` (3.3).

## No OpenAPI Registration Needed

Unlike Rust's utoipa (which requires manually adding paths + schemas to
`#[openapi(...)]`), springdoc-openapi **auto-discovers** all `@RestController`
classes and their `@Operation` / `@Tag` annotations. The OpenAPI JSON at
`/v3/api-docs` is always up-to-date. No file edits needed outside the new
controller.

## DTO Convention

- Request DTOs are `record` types named `{Action}{Feature}Request` (e.g. `CreateInvoiceRequest`)
- Response DTOs: if the entity is clean, return it directly. Otherwise create a `{Feature}Detail` or `{Feature}Summary` record.
- Query DTOs: for complex query params, create a `Query{Feature}s` record and use `@ModelAttribute`
- All request DTOs use `jakarta.validation.constraints` (`@NotBlank`, `@NotNull`, `@Min`, etc.)
- Records are immutable - springdoc auto-generates their schema from the component types

## URL Path Convention

- All business routes are under `/v1/{feature}s` (plural, kebab-case)
- The `context-path` (`{context_path}`) is prepended by the servlet container
- The api-gateway strips `/v1/{short}` and forwards to `{context_path}/v1/{feature}s`
- So the full external path is: `http://gateway:3000/v1/{short}/{feature}s`
- Internal path (for controller `@RequestMapping`): `/v1/{feature}s` (context-path handles the rest)

## Validation

After creating files, verify:

1. **Compiles**: `cd services/{name} && mvn -Pjava21 clean compile`
2. **OpenAPI reflects new route**: start the service and `curl http://localhost:{port}{context_path}/v3/api-docs | python3 -m json.tool | grep {feature}`
3. **Test passes**: `mvn -Pjava21 test -pl {short}-app-bootstrap -Dtest={Feature}ControllerTest`
4. **Service standard still passes**: `node scripts/check-service-standard.js` (from repo root)
5. **OpenAPI spec check**: `node scripts/check-spec.js` (validates operationId / summary presence)

## Common Mistakes to Avoid

- ❌ Putting the controller in the bootstrap module instead of the api module -> component scan may still find it, but module boundaries get blurred
- ❌ Forgetting `@Valid` on `@RequestBody` -> validation constraints silently ignored
- ❌ Using `@RequestBody` on GET endpoints -> Spring will reject it; use `@RequestParam` or `@ModelAttribute` instead
- ❌ Not creating the Flyway migration for the new table (use `coder-migrate-java-db` skill) -> entity queries fail at runtime with "table not found"
- ❌ Returning entities with sensitive fields (e.g. `deleted_at`, internal IDs) -> create a response DTO record and map in the controller
- ❌ Hardcoding the service URL in the controller -> the service is injected via constructor, never call `new {Feature}Service()`

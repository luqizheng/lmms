package com.coder.pm.api.kb;

import com.baomidou.mybatisplus.core.metadata.IPage;
import com.coder.pm.infra.persistence.entity.KbSpace;
import com.coder.pm.infra.service.KbSpaceService;
import io.swagger.v3.oas.annotations.Operation;
import io.swagger.v3.oas.annotations.tags.Tag;
import jakarta.validation.Valid;
import jakarta.validation.constraints.NotBlank;
import org.springframework.http.ResponseEntity;
import org.springframework.web.bind.annotation.DeleteMapping;
import org.springframework.web.bind.annotation.GetMapping;
import org.springframework.web.bind.annotation.PatchMapping;
import org.springframework.web.bind.annotation.PathVariable;
import org.springframework.web.bind.annotation.PostMapping;
import org.springframework.web.bind.annotation.RequestBody;
import org.springframework.web.bind.annotation.RequestMapping;
import org.springframework.web.bind.annotation.RequestParam;
import org.springframework.web.bind.annotation.RestController;

import java.util.Map;

/**
 * KB space lifecycle. Documents live under a space (see {@link DocumentController}).
 *
 * <pre>
 * POST   /pm-api/v1/spaces                       create
 * GET    /pm-api/v1/spaces                       paged list (?page&size&projectId&visibility)
 * GET    /pm-api/v1/spaces/{id}                  detail
 * PATCH  /pm-api/v1/spaces/{id}                  patch (name/description/visibility)
 * DELETE /pm-api/v1/spaces/{id}                  soft-delete
 * </pre>
 */
@RestController
@RequestMapping("/v1/spaces")
@Tag(name = "kb-spaces", description = "Knowledge base space lifecycle")
public class KbSpaceController {

    private final KbSpaceService service;

    public KbSpaceController(KbSpaceService service) {
        this.service = service;
    }

    @PostMapping
    @Operation(summary = "Create a KB space")
    public ResponseEntity<KbSpace> create(@RequestBody @Valid CreateSpaceRequest req) {
        KbSpace s = new KbSpace();
        s.setProjectId(req.projectId());
        s.setName(req.name());
        s.setDescription(req.description());
        s.setVisibility(req.visibility());
        s.setCreatedBy(0L); // populated from SecurityContext upstream if/when JWT auth wired
        KbSpace saved = service.create(s);
        return ResponseEntity.status(201).body(saved);
    }

    @GetMapping
    @Operation(summary = "Paged KB space list")
    public Map<String, Object> list(@RequestParam(defaultValue = "1") int page,
                                    @RequestParam(defaultValue = "20") int size,
                                    @RequestParam(required = false) Long projectId,
                                    @RequestParam(required = false) String visibility) {
        IPage<KbSpace> p = service.page(page, size, projectId, visibility);
        return Map.of(
                "items", p.getRecords(),
                "total", p.getTotal(),
                "page", p.getCurrent(),
                "size", p.getSize()
        );
    }

    @GetMapping("/{id}")
    @Operation(summary = "KB space detail")
    public KbSpace detail(@PathVariable long id) {
        return service.findActiveOrThrow(id);
    }

    @PatchMapping("/{id}")
    @Operation(summary = "Patch KB space")
    public KbSpace patch(@PathVariable long id, @RequestBody UpdateSpaceRequest req) {
        KbSpace patch = new KbSpace();
        patch.setName(req.name());
        patch.setDescription(req.description());
        patch.setVisibility(req.visibility());
        return service.update(id, patch);
    }

    @DeleteMapping("/{id}")
    @Operation(summary = "Soft-delete KB space")
    public ResponseEntity<Void> delete(@PathVariable long id) {
        KbSpace s = service.findActiveOrThrow(id);
        s.setDeletedAt(java.time.LocalDateTime.now());
        service.updateById(s);
        return ResponseEntity.noContent().build();
    }

    public record CreateSpaceRequest(
            Long projectId,
            @NotBlank String name,
            String description,
            String visibility) {}

    public record UpdateSpaceRequest(
            String name,
            String description,
            String visibility) {}
}

package com.coder.pm.api.story;

import com.coder.pm.infra.persistence.entity.UserStory;
import com.coder.pm.infra.service.RequirementService;
import com.coder.pm.infra.service.UserStoryService;
import io.swagger.v3.oas.annotations.Operation;
import io.swagger.v3.oas.annotations.tags.Tag;
import jakarta.validation.Valid;
import jakarta.validation.constraints.NotNull;
import org.springframework.http.ResponseEntity;
import org.springframework.web.bind.annotation.GetMapping;
import org.springframework.web.bind.annotation.PatchMapping;
import org.springframework.web.bind.annotation.PathVariable;
import org.springframework.web.bind.annotation.PostMapping;
import org.springframework.web.bind.annotation.RequestBody;
import org.springframework.web.bind.annotation.RequestMapping;
import org.springframework.web.bind.annotation.RestController;

import java.util.List;

/**
 * User-story lifecycle. Nested under a requirement for create + list; flat for detail / patch.
 */
@RestController
@Tag(name = "stories", description = "User stories grouped under a requirement")
public class UserStoryController {

    private final UserStoryService service;
    private final RequirementService requirementService;

    public UserStoryController(UserStoryService service, RequirementService requirementService) {
        this.service = service;
        this.requirementService = requirementService;
    }

    @PostMapping("/v1/requirements/{requirementId}/stories")
    @Operation(summary = "Append story to requirement")
    public ResponseEntity<UserStory> create(@PathVariable long requirementId,
                                           @RequestBody @Valid CreateStoryRequest req) {
        requirementService.findOrThrow(requirementId);
        UserStory s = new UserStory();
        s.setRequirementId(requirementId);
        s.setAsA(req.asA());
        s.setIWant(req.iWant());
        s.setSoThat(req.soThat());
        s.setStoryPoints(req.storyPoints());
        s.setStatus(req.status());
        s.setSortOrder(req.sortOrder());
        UserStory saved = service.create(s);
        return ResponseEntity.status(201).body(saved);
    }

    @GetMapping("/v1/requirements/{requirementId}/stories")
    @Operation(summary = "List stories for requirement")
    public List<UserStory> list(@PathVariable long requirementId) {
        return service.listByRequirement(requirementId);
    }

    @GetMapping("/v1/stories/{id}")
    @Operation(summary = "Story detail")
    public UserStory detail(@PathVariable long id) {
        return service.findOrThrow(id);
    }

    @PatchMapping("/v1/stories/{id}")
    @Operation(summary = "Patch story fields")
    public UserStory patch(@PathVariable long id, @RequestBody UpdateStoryRequest req) {
        UserStory p = new UserStory();
        p.setAsA(req.asA());
        p.setIWant(req.iWant());
        p.setSoThat(req.soThat());
        p.setStoryPoints(req.storyPoints());
        p.setStatus(req.status());
        p.setSortOrder(req.sortOrder());
        return service.update(id, p);
    }

    public record CreateStoryRequest(
            String asA,
            String iWant,
            String soThat,
            Integer storyPoints,
            String status,
            Integer sortOrder
    ) {}

    public record UpdateStoryRequest(
            String asA,
            String iWant,
            String soThat,
            @NotNull Integer storyPoints,
            String status,
            Integer sortOrder
    ) {}
}

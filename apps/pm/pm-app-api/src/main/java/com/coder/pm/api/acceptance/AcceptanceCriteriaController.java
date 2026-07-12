package com.coder.pm.api.acceptance;

import com.coder.pm.infra.persistence.entity.AcceptanceCriteria;
import com.coder.pm.infra.service.AcceptanceCriteriaService;
import com.coder.pm.infra.service.UserStoryService;
import io.swagger.v3.oas.annotations.Operation;
import io.swagger.v3.oas.annotations.tags.Tag;
import jakarta.validation.Valid;
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
 * Acceptance criteria (Given / When / Then) lifecycle. Grouped under a user story.
 */
@RestController
@Tag(name = "criteria", description = "Acceptance criteria (Given/When/Then) under a story")
public class AcceptanceCriteriaController {

    private final AcceptanceCriteriaService service;
    private final UserStoryService userStoryService;

    public AcceptanceCriteriaController(AcceptanceCriteriaService service,
                                        UserStoryService userStoryService) {
        this.service = service;
        this.userStoryService = userStoryService;
    }

    @PostMapping("/v1/stories/{storyId}/criteria")
    @Operation(summary = "Append acceptance criterion to story")
    public ResponseEntity<AcceptanceCriteria> create(@PathVariable long storyId,
                                                     @RequestBody @Valid CreateAcceptanceRequest req) {
        userStoryService.findOrThrow(storyId);
        AcceptanceCriteria a = new AcceptanceCriteria();
        a.setUserStoryId(storyId);
        a.setGivenText(req.givenText());
        a.setWhenText(req.whenText());
        a.setThenText(req.thenText());
        a.setChecked(req.checked());
        a.setSortOrder(req.sortOrder());
        AcceptanceCriteria saved = service.create(a);
        return ResponseEntity.status(201).body(saved);
    }

    @GetMapping("/v1/stories/{storyId}/criteria")
    @Operation(summary = "List criteria for story")
    public List<AcceptanceCriteria> list(@PathVariable long storyId) {
        return service.listByStory(storyId);
    }

    @PatchMapping("/v1/criteria/{id}")
    @Operation(summary = "Patch criterion fields")
    public AcceptanceCriteria patch(@PathVariable long id, @RequestBody UpdateAcceptanceRequest req) {
        AcceptanceCriteria p = new AcceptanceCriteria();
        p.setGivenText(req.givenText());
        p.setWhenText(req.whenText());
        p.setThenText(req.thenText());
        p.setChecked(req.checked());
        p.setSortOrder(req.sortOrder());
        return service.update(id, p);
    }

    @PostMapping("/v1/criteria/{id}/check")
    @Operation(summary = "Toggle checked status")
    public AcceptanceCriteria check(@PathVariable long id, @RequestBody CheckRequest req) {
        boolean v = req.checked() == null ? true : req.checked();
        return service.check(id, v);
    }

    public record CreateAcceptanceRequest(
            String givenText,
            String whenText,
            String thenText,
            Boolean checked,
            Integer sortOrder
    ) {}

    public record UpdateAcceptanceRequest(
            String givenText,
            String whenText,
            String thenText,
            Boolean checked,
            Integer sortOrder
    ) {}

    public record CheckRequest(Boolean checked) {}
}

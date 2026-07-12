package com.coder.pm.api.comment;

import com.coder.pm.infra.persistence.entity.Comment;
import com.coder.pm.infra.service.CommentService;
import io.swagger.v3.oas.annotations.Operation;
import io.swagger.v3.oas.annotations.tags.Tag;
import jakarta.validation.Valid;
import jakarta.validation.constraints.NotBlank;
import org.springframework.http.ResponseEntity;
import org.springframework.web.bind.annotation.*;

import java.util.List;

@RestController
@Tag(name = "comments", description = "KB document comments (Phase D7)")
public class CommentController {

    private final CommentService service;

    public CommentController(CommentService service) {
        this.service = service;
    }

    @PostMapping("/v1/documents/{documentId}/comments")
    @Operation(summary = "Add comment to document")
    public ResponseEntity<Comment> create(@PathVariable long documentId,
                                         @RequestBody @Valid CreateCommentRequest req) {
        Comment c = new Comment();
        c.setDocumentId(documentId);
        c.setContent(req.content());
        c.setParentId(req.parentId());
        c.setAuthorId(req.authorId());
        Comment saved = service.create(c);
        return ResponseEntity.status(201).body(saved);
    }

    @GetMapping("/v1/documents/{documentId}/comments")
    @Operation(summary = "List comments for document")
    public List<Comment> list(@PathVariable long documentId) {
        return service.listByDocument(documentId);
    }

    public record CreateCommentRequest(
            @NotBlank String content,
            Long parentId,
            Long authorId
    ) {}
}

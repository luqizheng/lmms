package com.coder.pm.api.attachment;

import com.coder.pm.infra.service.AttachmentService;
import com.kudi.coder.filesdk.model.FileInfo;
import io.swagger.v3.oas.annotations.Operation;
import io.swagger.v3.oas.annotations.tags.Tag;
import org.springframework.http.ResponseEntity;
import org.springframework.web.bind.annotation.*;
import org.springframework.web.multipart.MultipartFile;

import java.io.IOException;
import java.util.Map;

/**
 * File upload/download endpoints. Phase D6: MVP supports single-file upload
 * with an optional business tag (e.g. {@code "kb:doc:42"}). The actual file
 * content is stored in file-service; this controller only passes through.
 *
 * <p>Note: file-service calls currently 401 because the SDK bearer is
 * {@code dev-bypass-token}, not a valid RS256 JWT. The code path is correct;
 * Phase G will inject a service-to-service JWT to unblock it.
 */
@RestController
@Tag(name = "attachments", description = "File attachment upload (Phase D6)")
public class AttachmentController {

    private final AttachmentService attachmentService;

    public AttachmentController(AttachmentService attachmentService) {
        this.attachmentService = attachmentService;
    }

    @PostMapping("/v1/attachments/upload")
    @Operation(summary = "Upload a file attachment")
    public ResponseEntity<FileInfo> upload(
            @RequestParam("file") MultipartFile file,
            @RequestParam(value = "tag", required = false) String tag,
            @RequestParam(value = "tenantId", defaultValue = "1") int tenantId) throws IOException {
        byte[] content = file.getBytes();
        String fileName = file.getOriginalFilename();
        FileInfo info = attachmentService.upload(fileName, content, tag, tenantId);
        return ResponseEntity.ok(info);
    }
}

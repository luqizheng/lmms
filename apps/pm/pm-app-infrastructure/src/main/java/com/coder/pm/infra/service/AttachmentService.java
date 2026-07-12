package com.coder.pm.infra.service;

import com.kudi.coder.filesdk.FileServiceClient;
import com.kudi.coder.filesdk.model.FileInfo;
import com.kudi.coder.filesdk.model.FileQuery;
import com.kudi.coder.filesdk.model.PageResult;
import com.kudi.coder.filesdk.model.UploadFileMeta;
import org.springframework.stereotype.Service;

/**
 * Attachment service — thin wrapper around {@link FileServiceClient} that
 * keeps the SDK dependency isolated from the rest of the PM domain code.
 *
 * <p>Phase D6: MVP supports single-file upload + metadata query. The SDK
 * bearer is currently {@code pm.sdk.bearer.static-token} (dev-bypass-token),
 * which is <strong>not</strong> a valid RS256 JWT — real file-service calls
 * will 401 until Phase G injects a proper service-to-service JWT. The code
 * path is correct; the wiring just needs the token swap.
 */
@Service
public class AttachmentService {

    private final FileServiceClient client;

    public AttachmentService(FileServiceClient client) {
        this.client = client;
    }

    /**
     * Upload a file. Accepts any content type; the service handles raw bytes.
     *
     * @param fileName    display name (used as the multipart filename)
     * @param content     raw file bytes
     * @param tag         optional business tag (e.g. {@code "kb:doc:42"})
     * @param tenantId    tenant id (1 for single-tenant MVP)
     * @return {@link FileInfo} with the server-assigned id and metadata
     */
    public FileInfo upload(String fileName, byte[] content, String tag, String tenantId) {
        UploadFileMeta meta = new UploadFileMeta();
        meta.setName(fileName);
        meta.setTag(tag);
        meta.setTenantId(tenantId);
        return client.uploadFile(meta, content);
    }

    public FileInfo getInfo(long fileId) {
        return client.getFileInfo(fileId);
    }

    public void delete(long fileId) {
        client.deleteFile(fileId);
    }

    public PageResult<FileInfo> query(String tag, int page, int size) {
        FileQuery q = new FileQuery();
        q.setTag(tag);
        q.setPage(page);
        q.setSize(size);
        return client.queryFiles(q);
    }
}

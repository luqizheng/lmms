export interface FileInfo {
    id: string;
    filename: string;
    size: number;
    /** Internal object key; do not expose to end users. */
    storage_path: string;
    tenant_id: number;
    is_temporary: boolean;
    created_at: string;
    updated_at: string;
    content_type: string | null;
    hash: string | null;
    user_id: string | null;
    expires_at: string | null;
    /** @deprecated Legacy compatibility field. User downloads must use a business-service-authorized presigned URL. */
    url: string;
}
export interface FileQuery {
    tenant_id?: number;
    user_id?: string;
    filename?: string;
    page?: number;
    size?: number;
}
export interface PagedFiles {
    items: FileInfo[];
    total: number;
    page: number;
    size: number;
}
/** Multipart metadata. Temporary files default to a 24-hour TTL unless explicitly set. */
export interface UploadFileMeta {
    filename: string;
    content_type?: string | null;
    tenant_id?: number | null;
    user_id?: string | null;
    is_temporary?: boolean | null;
    expires_in_seconds?: number | null;
}
export type PresignOperation = "get" | "put";
export interface CreatePresignUrl {
    file_id: string;
    operation: PresignOperation;
    /** Valid range: 1..604800 seconds (7 days). */
    expires_in_seconds?: number | null;
}
export interface PresignUrlResponse {
    url: string;
    expires_at: string;
    operation: PresignOperation;
}
export interface DownloadResult {
    bytes: ArrayBuffer;
    contentType: string;
    filename: string | null;
    contentDisposition: string | null;
}
//# sourceMappingURL=types.d.ts.map
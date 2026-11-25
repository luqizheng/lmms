// Internal presign API. Requires service-jwt (aud=file-service).
export class FilePresignApi {
    http;
    constructor(http) {
        this.http = http;
    }
    /** Create a short-lived GET or PUT URL: POST /file/presign. */
    create(request) {
        return this.http.post("/file/presign", request, { auth: true });
    }
}
//# sourceMappingURL=presign.js.map
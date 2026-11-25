import type { HttpCore } from "./http.js";
import type { CreatePresignUrl, PresignUrlResponse } from "./types.js";
export declare class FilePresignApi {
    private readonly http;
    constructor(http: HttpCore);
    /** Create a short-lived GET or PUT URL: POST /file/presign. */
    create(request: CreatePresignUrl): Promise<PresignUrlResponse>;
}
//# sourceMappingURL=presign.d.ts.map
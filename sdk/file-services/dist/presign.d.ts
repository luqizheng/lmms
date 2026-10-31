import type { HttpCore } from "./http.js";
import type { CreatePresignUrl, PresignUrlResponse } from "./types.js";
export declare class FilePresignApi {
    private readonly http;
    constructor(http: HttpCore);
    /** 创建预签名 URL：POST /v1/file/presign。过期返回 401 "presign expired"。 */
    create(request: CreatePresignUrl): Promise<PresignUrlResponse>;
}
//# sourceMappingURL=presign.d.ts.map
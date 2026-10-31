// 预签名 URL（file-service，经网关 /v1/file/presign，需要鉴权）。
export class FilePresignApi {
    http;
    constructor(http) {
        this.http = http;
    }
    /** 创建预签名 URL：POST /v1/file/presign。过期返回 401 "presign expired"。 */
    create(request) {
        return this.http.post("/v1/file/presign", request, { auth: true });
    }
}
//# sourceMappingURL=presign.js.map
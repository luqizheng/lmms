export interface ApiResponse<T> {
    /** 0 = 成功；错误时镜像 HTTP 状态码 */
    code: number;
    message: string;
    data: T;
}
/** 解包 ApiResponse：非 0 抛错（错误体保留在 error.body，409 时含 current_etag 等）。 */
export declare function unwrap<T>(call: Promise<ApiResponse<T>>): Promise<T>;
//# sourceMappingURL=envelope.d.ts.map
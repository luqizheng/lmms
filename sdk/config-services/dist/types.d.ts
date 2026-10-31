export interface Namespace {
    id: string;
    tenant_id: number;
    app: string;
    env: string;
    profile: string | null;
    description: string | null;
    created_at: string;
    updated_at: string;
}
export interface CreateNamespace {
    app: string;
    env: string;
    profile?: string;
    description?: string;
}
export interface UpdateNamespace {
    description?: string;
}
export interface NamespaceQuery {
    app?: string;
    env?: string;
    page?: number;
    size?: number;
}
export interface PagedNamespaces {
    items: Namespace[];
    total: number;
    page: number;
    size: number;
}
export interface ConfigItem {
    id: string;
    namespace_id: string;
    key: string;
    value: string;
    value_type: string;
    is_secret: boolean;
    created_at: string;
    updated_at: string;
    description: string | null;
    /** 服务端注入：is_secret 且调用方无 CFG_CONFIG_REVEAL 权限时为 true，value 为 "********" */
    value_masked: boolean;
}
export interface CreateConfigItem {
    namespace_id: string;
    key: string;
    value: string;
    value_type?: string;
    is_secret?: boolean;
    description?: string;
}
export interface UpdateConfigItem {
    value?: string;
    value_type?: string;
    description?: string;
    is_secret?: boolean;
}
export interface ConfigItemQuery {
    namespace_id?: string;
    key?: string;
    page?: number;
    size?: number;
}
export interface PagedConfigItems {
    items: ConfigItem[];
    total: number;
    page: number;
    size: number;
}
/** POST /v1/config/import/{namespace_id} 的响应。 */
export interface ImportResult {
    success: boolean;
    count: number;
}
/**
 * GET /v1/config/watch 的响应。注意：上游当前是长轮询 stub——
 * 睡眠 timeout 后恒返回 {changed:false, data:[]}，勿据此做推送。
 */
export interface WatchResult {
    namespace_id: string | null;
    changed: boolean;
    data: unknown[];
}
//# sourceMappingURL=types.d.ts.map
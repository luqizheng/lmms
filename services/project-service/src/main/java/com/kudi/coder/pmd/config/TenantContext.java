package com.kudi.coder.pmd.config;

/**
 * ThreadLocal 持有请求级租户上下文。
 * 由 {@link TenantContextFilter} 写入；响应前清理（filter 的 finally 块）。
 */
public final class TenantContext {

    private static final ThreadLocal<Long> TENANT_ID = new ThreadLocal<>();
    private static final ThreadLocal<String> USER_ID = new ThreadLocal<>();

    private TenantContext() {}

    public static void setTenantId(Long tenantId) {
        TENANT_ID.set(tenantId);
    }

    public static Long requireTenantId() {
        Long t = TENANT_ID.get();
        if (t == null) {
            throw new IllegalStateException("tenant context missing (handler invoked outside JWT scope)");
        }
        return t;
    }

    public static Long getTenantIdOrNull() {
        return TENANT_ID.get();
    }

    public static void setUserId(String userId) {
        USER_ID.set(userId);
    }

    public static String getUserId() {
        return USER_ID.get();
    }

    public static void clear() {
        TENANT_ID.remove();
        USER_ID.remove();
    }
}

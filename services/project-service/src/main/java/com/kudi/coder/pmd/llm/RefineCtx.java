package com.kudi.coder.pmd.llm;

/**
 * Per-call context passed to {@link LlmProvider#refineRequirements}.
 * Reads the thread-local tenant via {@code TenantContext} at controller
 * boundary but the call-site explicitly threads it here so test code can
 * inject without the filter.
 */
public record RefineCtx(Long tenantId, String callerUserId, Long projectId) { }

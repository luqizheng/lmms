package com.kudi.coder.pmd.llm;

import com.kudi.coder.pmd.web.AppException;

/**
 * Abstraction over an LLM refinement provider. Implementations MUST be
 * deterministic for the same input (or carry their own de-duplication) and
 * MUST NOT throw non-recoverable exceptions — failures should map to
 * {@code PMD_LLM_REFINE_FORBIDDEN} via {@link AppException#forbidden}.
 *
 * MVP binding: {@link PlaceholderLlmProvider}.
 * Future binding: an HTTP-backed provider (Ark / Ollama / etc.) — env
 * {@code pmd.llm.provider=http} selects it via {@link LlmConfig}.
 */
public interface LlmProvider {
    RefinedRequirements refineRequirements(String prompt, RefineCtx ctx);
}

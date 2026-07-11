package com.kudi.coder.pmd.llm;

import org.springframework.boot.autoconfigure.condition.ConditionalOnProperty;
import org.springframework.stereotype.Component;

/**
 * Default {@link LlmProvider} — no network call, deterministic canned response.
 * Truncates the input prompt to 120 chars and emits a single item with code "P-1".
 *
 * Selected by default (env {@code pmd.llm.provider} unset / "placeholder").
 * The {@code @ConditionalOnProperty} gate lets future providers turn off the
 * placeholder via env without code changes.
 */
@Component
@ConditionalOnProperty(name = "pmd.llm.provider", havingValue = "placeholder", matchIfMissing = true)
public class PlaceholderLlmProvider implements LlmProvider {

    private static final int MAX_SUMMARY_CHARS = 120;

    @Override
    public RefinedRequirements refineRequirements(String prompt, RefineCtx ctx) {
        if (prompt == null || prompt.isBlank()) {
            return new RefinedRequirements(java.util.List.of());
        }
        String summary = prompt.length() > MAX_SUMMARY_CHARS
                ? prompt.substring(0, MAX_SUMMARY_CHARS)
                : prompt;
        return new RefinedRequirements(java.util.List.of(new RefinedItem("P-1", summary)));
    }
}

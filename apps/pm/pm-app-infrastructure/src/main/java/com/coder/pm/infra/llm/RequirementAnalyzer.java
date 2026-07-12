package com.coder.pm.infra.llm;

import com.coder.pm.infra.service.RequirementService;
import com.coder.pm.infra.persistence.entity.Requirement;
import com.coder.pm.llm.CompletionRequest;
import com.coder.pm.llm.CompletionResponse;
import com.coder.pm.llm.LlmProviderRegistry;
import org.springframework.stereotype.Service;

/**
 * Wraps {@link LlmProviderRegistry} with domain-specific prompts:
 * <ul>
 *   <li>{@link #decompose(long)} — turn a requirement into user stories + AC via an LLM.</li>
 *   <li>{@link #refine(long)} — rewrite a requirement's description for clarity.</li>
 * </ul>
 *
 * <p>When the chosen provider is {@code noop} (MVP default), responses carry
 * {@code disabled=true} so the frontend can render a disabled UI hint.
 */
@Service
public class RequirementAnalyzer {

    private final LlmProviderRegistry registry;
    private final RequirementService requirementService;

    public RequirementAnalyzer(LlmProviderRegistry registry,
                               RequirementService requirementService) {
        this.registry = registry;
        this.requirementService = requirementService;
    }

    public CompletionResponse decompose(long requirementId) {
        Requirement r = requirementService.findOrThrow(requirementId);
        return registry.dispatch(defaultProvider(), CompletionRequest.builder()
                .prompt(buildDecomposePrompt(r))
                .systemPrompt(DECOMPOSE_SYSTEM)
                .maxTokens(1024)
                .temperature(0.3)
                .build());
    }

    public CompletionResponse refine(long requirementId) {
        Requirement r = requirementService.findOrThrow(requirementId);
        return registry.dispatch(defaultProvider(), CompletionRequest.builder()
                .prompt("Rewrite this requirement so it is unambiguous and testable:\n\n" + safe(r.getDescription()))
                .systemPrompt(REFINE_SYSTEM)
                .maxTokens(512)
                .temperature(0.2)
                .build());
    }

    private String defaultProvider() {
        return System.getenv().getOrDefault("PM_LLM_PROVIDER", "noop");
    }

    private static String buildDecomposePrompt(Requirement r) {
        return "Title: " + safe(r.getTitle()) + "\n\nDescription:\n" + safe(r.getDescription());
    }

    private static String safe(String s) { return s == null ? "" : s; }

    private static final String DECOMPOSE_SYSTEM = """
            You are a senior software requirements analyst.
            Decompose the supplied requirement into 3–7 user stories in the form
            "As a <role>, I want <capability>, so that <benefit>", and 1–3
            acceptance criteria per story in Given / When / Then.
            Output strict JSON: { "stories": [ { "title": "...", "i_want": "...",
            "so_that": "...", "criteria": [ { "given": "...", "when": "...",
            "then": "..." } ] } ] }. No prose outside the JSON.
            """;

    private static final String REFINE_SYSTEM = """
            You rewrite ambiguous or vague product requirements.
            Keep the original intent; clarify actors, scope, and acceptance.
            Output strictly the refined description, no preamble.
            """;
}

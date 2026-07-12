package com.coder.pm.api.llm;

import com.coder.pm.infra.llm.RequirementAnalyzer;
import com.coder.pm.llm.CompletionResponse;
import com.coder.pm.llm.LlmProviderRegistry;
import io.swagger.v3.oas.annotations.Operation;
import io.swagger.v3.oas.annotations.tags.Tag;
import org.springframework.web.bind.annotation.GetMapping;
import org.springframework.web.bind.annotation.PathVariable;
import org.springframework.web.bind.annotation.PostMapping;
import org.springframework.web.bind.annotation.RequestMapping;
import org.springframework.web.bind.annotation.RequestParam;
import org.springframework.web.bind.annotation.RestController;

import java.util.List;

/**
 * LLM-assisted endpoints for requirements. Always returns a {@link CompletionResponse}; when the
 * active provider is {@code noop} (MVP default), the response carries {@code disabled=true} so
 * callers can degrade gracefully.
 *
 * <pre>
 * GET  /pm-api/v1/llm/providers                        list of registered provider names
 * POST /pm-api/v1/llm/requirements/{id}/decompose      decompose requirement into stories + AC
 * POST /pm-api/v1/llm/requirements/{id}/refine        rewrite requirement description
 * </pre>
 */
@RestController
@RequestMapping("/v1/llm")
@Tag(name = "llm", description = "LLM-assisted requirement analysis")
public class LlmController {

    private final LlmProviderRegistry registry;
    private final RequirementAnalyzer analyzer;

    public LlmController(LlmProviderRegistry registry, RequirementAnalyzer analyzer) {
        this.registry = registry;
        this.analyzer = analyzer;
    }

    @GetMapping("/providers")
    @Operation(summary = "List registered LLM provider names")
    public List<String> providers() {
        return registry.listProviderNames();
    }

    @PostMapping("/requirements/{id}/decompose")
    @Operation(summary = "Decompose a requirement into user stories + acceptance criteria")
    public CompletionResponse decompose(@PathVariable long id,
                                        @RequestParam(required = false) String provider) {
        return (provider == null || provider.isBlank())
                ? analyzer.decompose(id)
                : analyzer.decompose(id, provider);
    }

    @PostMapping("/requirements/{id}/refine")
    @Operation(summary = "Rewrite a requirement's description for clarity")
    public CompletionResponse refine(@PathVariable long id,
                                     @RequestParam(required = false) String provider) {
        return (provider == null || provider.isBlank())
                ? analyzer.refine(id)
                : analyzer.refine(id, provider);
    }
}

package com.kudi.coder.pmd.llm;

import com.kudi.coder.pmd.config.TenantContext;
import com.kudi.coder.pmd.outbox.OutboxWriter;
import com.kudi.coder.pmd.web.AppException;
import io.swagger.v3.oas.annotations.Operation;
import io.swagger.v3.oas.annotations.tags.Tag;
import jakarta.validation.Valid;
import jakarta.validation.constraints.NotBlank;
import org.springframework.web.bind.annotation.PostMapping;
import org.springframework.web.bind.annotation.RequestBody;
import org.springframework.web.bind.annotation.RequestMapping;
import org.springframework.web.bind.annotation.RestController;

import java.util.HashMap;
import java.util.List;
import java.util.Map;

/**
 * POST /llm/refine — refine raw requirements text into structured items
 * via the configured {@link LlmProvider}. Side effects (audit log row +
 * outbox event) are emitted in the same {@code @Transactional} boundary so
 * either both rows land or neither does.
 */
@RestController
@RequestMapping("/llm")
@Tag(name = "llm", description = "PMD LLM-assisted endpoints")
public class LlmRefineController {

    private static final String ROUTING_KEY = "pmd.llm.refinement.requested";
    private static final String PROVIDER_PLACEHOLDER = "placeholder";

    private final LlmProvider provider;
    private final OutboxWriter outbox;

    public LlmRefineController(LlmProvider provider, OutboxWriter outbox) {
        this.provider = provider;
        this.outbox = outbox;
    }

    @PostMapping("/refine")
    @Operation(summary = "Refine raw requirements text via the active LLM provider")
    public RefineResponse refine(@Valid @RequestBody RefineRequest body) {
        Long tenantId = TenantContext.requireTenantId();
        String caller = TenantContext.getUserId();
        RefineCtx ctx = new RefineCtx(tenantId, caller, body.projectId());

        RefinedRequirements refined = provider.refineRequirements(body.prompt(), ctx);

        // Envelope for the audit trail + downstream consumers of
        // pmd.llm.refinement.requested (audit-service records this).
        Map<String, Object> envelope = new HashMap<>();
        envelope.put("tenant_id", tenantId);
        envelope.put("project_id", body.projectId());
        envelope.put("caller_user_id", caller);
        envelope.put("provider", PROVIDER_PLACEHOLDER);
        envelope.put("prompt_summary", firstNChars(body.prompt(), 120));
        envelope.put("items_count", refined.items().size());

        outbox.enqueueWithLog(
                ROUTING_KEY,
                envelope,
                body.projectId(),
                /*taskId*/ null,
                body.prompt(),
                /*responseText*/ toJson(refined.items()));

        return new RefineResponse(refined.items());
    }

    private static String firstNChars(String s, int n) {
        return s.length() <= n ? s : s.substring(0, n);
    }

    private static String toJson(List<RefinedItem> items) {
        // Hand-rolled to avoid pulling a JSON dep into a hot path that's
        // already serializing through Jackson elsewhere.
        StringBuilder sb = new StringBuilder("[");
        for (int i = 0; i < items.size(); i++) {
            if (i > 0) sb.append(',');
            RefinedItem it = items.get(i);
            sb.append("{\"code\":\"").append(escape(it.code()))
                    .append("\",\"summary\":\"").append(escape(it.summary()))
                    .append("\"}");
        }
        return sb.append(']').toString();
    }

    private static String escape(String s) {
        if (s == null) return "";
        return s.replace("\\", "\\\\").replace("\"", "\\\"");
    }

    public record RefineRequest(
            @NotBlank String prompt,
            Long projectId) { }

    public record RefineResponse(List<RefinedItem> items) { }
}

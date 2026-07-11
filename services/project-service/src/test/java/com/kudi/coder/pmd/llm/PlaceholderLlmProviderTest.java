package com.kudi.coder.pmd.llm;

import com.kudi.coder.pmd.config.TenantContext;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * Pure unit test for the placeholder provider. No Spring context, no DB, no
 * network — confirms the canned behaviour the outbox envelope relies on.
 */
class PlaceholderLlmProviderTest {

    private final LlmProvider provider = new PlaceholderLlmProvider();

    @BeforeEach
    void setupTenantContext() {
        TenantContext.setTenantId(1L);
        TenantContext.setUserId("alice");
    }

    @AfterEach
    void clearTenantContext() {
        TenantContext.clear();
    }

    @Test
    void shortPromptProducesSingleItemWithCodeP1() {
        RefinedRequirements r = provider.refineRequirements(
                "Add pagination to the projects list.",
                new RefineCtx(1L, "alice", 42L));
        assertNotNull(r);
        assertEquals(1, r.items().size());
        assertEquals("P-1", r.items().get(0).code());
        assertEquals("Add pagination to the projects list.",
                r.items().get(0).summary());
    }

    @Test
    void longPromptIsTruncatedAt120Chars() {
        StringBuilder longPrompt = new StringBuilder();
        for (int i = 0; i < 300; i++) longPrompt.append('a');
        RefinedRequirements r = provider.refineRequirements(
                longPrompt.toString(),
                new RefineCtx(1L, "alice", 42L));
        assertEquals(1, r.items().size());
        assertTrue(r.items().get(0).summary().length() <= 120,
                "summary should be <= 120 chars, was "
                        + r.items().get(0).summary().length());
    }

    @Test
    void nullPromptProducesEmptyResult() {
        RefinedRequirements r = provider.refineRequirements(null,
                new RefineCtx(1L, "alice", null));
        assertNotNull(r);
        assertTrue(r.items().isEmpty());
    }

    @Test
    void blankPromptProducesEmptyResult() {
        RefinedRequirements r = provider.refineRequirements("   ",
                new RefineCtx(1L, "alice", null));
        assertTrue(r.items().isEmpty());
    }
}

package com.kudi.coder.pmd.llm;

import java.util.List;

/**
 * Result of refining raw requirements text. The {@code code} field is a
 * short slug suitable for downstream task creation; {@code summary} is the
 * pre-truncated version of the input (placeholder provider) or a real LLM
 * rephrasing (future HTTP provider).
 */
public record RefinedRequirements(List<RefinedItem> items) { }

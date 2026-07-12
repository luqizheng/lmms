package com.coder.pm.infra.markdown;

import com.vladsch.flexmark.ext.gfm.tasklist.TaskListExtension;
import com.vladsch.flexmark.ext.tables.TablesExtension;
import com.vladsch.flexmark.html.HtmlRenderer;
import com.vladsch.flexmark.parser.Parser;
import com.vladsch.flexmark.util.data.MutableDataSet;
import com.vladsch.flexmark.util.misc.Extension;
import org.springframework.stereotype.Component;

import java.util.Arrays;

/**
 * Markdown → HTML renderer. Backed by flexmark-java 0.64.8 with GFM tables + task lists.
 *
 * <p>Note on packages: in flexmark-java 0.64.x the GFM tables extension lives in
 * {@code com.vladsch.flexmark.ext.tables} (no {@code .gfm.} infix), while task lists live in
 * {@code com.vladsch.flexmark.ext.gfm.tasklist}. Verified against the published 0.64.8 jars.
 *
 * <p>Sanitization: we do <strong>not</strong> sanitize output here; rendering happens server-side
 * and is stored in {@code kb_document_versions.content_html}. The frontend should render via a
 * component that sanitizes (DOMPurify) before injecting into the live DOM.
 */
@Component
public class MarkdownRenderer {

    private final Parser parser;
    private final HtmlRenderer renderer;

    public MarkdownRenderer() {
        MutableDataSet opts = new MutableDataSet();
        opts.set(Parser.EXTENSIONS, Arrays.asList(
                TablesExtension.create(),
                TaskListExtension.create()));
        this.parser = Parser.builder(opts).build();
        this.renderer = HtmlRenderer.builder(opts).build();
    }

    public String render(String markdown) {
        if (markdown == null) return "";
        return renderer.render(parser.parse(markdown));
    }
}

<script setup lang="ts">
import { computed } from "vue";
import { Separator as RekaSeparator } from "reka-ui";
import { cn } from "@/lib/cn";

/**
 * reka-ui Separator 负责 role=separator / aria-orientation（Phase 1R §2.4），
 * 本组件只负责 PRISM 的视觉规格与 strong 档切换。
 */

interface Props {
  /** 分隔线方向 — Phase 1R §2.4 */
  orientation?: "horizontal" | "vertical";
  /** 强档描边（--border-default）；默认弱档（--border-subtle） */
  strong?: boolean;
  /** 纯装饰：不作为语义分隔线暴露给读屏 */
  decorative?: boolean;
}

const props = withDefaults(defineProps<Props>(), {
  orientation: "horizontal",
  strong: false,
  decorative: false
});

const classes = computed(() =>
  cn("ui-separator", `ui-separator--${props.orientation}`, {
    "ui-separator--strong": props.strong
  })
);
</script>

<template>
  <RekaSeparator
    :orientation="orientation"
    :decorative="decorative"
    :class="classes"
  />
</template>

<style scoped>
/* 分隔线 — 1px 细线，默认弱档 --border-subtle */
.ui-separator {
  flex: none;
  border: 0;
  margin: 0;
  padding: 0;
  background: var(--border-subtle);
}

/* 强档 — --border-default */
.ui-separator--strong {
  background: var(--border-default);
}

.ui-separator--horizontal {
  width: 100%;
  height: var(--hairline);
}

.ui-separator--vertical {
  width: var(--hairline);
  height: 100%;
  align-self: stretch;
}
</style>

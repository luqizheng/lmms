<script setup lang="ts">
import {
  ScrollAreaRoot,
  ScrollAreaScrollbar,
  ScrollAreaThumb,
  ScrollAreaViewport
} from "reka-ui";

interface Props {
  /** 滚动方向 */
  orientation?: "vertical" | "horizontal";
}

withDefaults(defineProps<Props>(), { orientation: "vertical" });
</script>

<template>
  <ScrollAreaRoot class="ui-scroll-area" type="auto">
    <ScrollAreaViewport class="ui-scroll-area__viewport">
      <slot />
    </ScrollAreaViewport>
    <ScrollAreaScrollbar
      class="ui-scroll-area__scrollbar"
      :orientation="orientation"
      force-mount
    >
      <ScrollAreaThumb class="ui-scroll-area__thumb" force-mount />
    </ScrollAreaScrollbar>
  </ScrollAreaRoot>
</template>

<style scoped>
.ui-scroll-area {
  position: relative;
  overflow: hidden;
}

/* reka-ui 的视口 / 滚动条 / 滑块在组件内部渲染，需 :deep() 命中 */
:deep(.ui-scroll-area__viewport) {
  width: 100%;
  height: 100%;
  overflow: auto;
}

:deep(.ui-scroll-area__scrollbar) {
  display: flex;
  user-select: none;
  touch-action: none;
  padding: var(--space-1);
  background: transparent;
  transition: opacity var(--duration-fast) var(--ease-decelerate);
}

:deep(.ui-scroll-area__scrollbar[data-orientation="vertical"]) {
  width: var(--space-2);
}

:deep(.ui-scroll-area__scrollbar[data-orientation="horizontal"]) {
  height: var(--space-2);
  flex-direction: row;
}

:deep(.ui-scroll-area__thumb) {
  flex: 1;
  border-radius: var(--radius-full);
  background: var(--border-strong);
  transition: background-color var(--duration-fast) var(--ease-decelerate);
}

:deep(.ui-scroll-area__scrollbar:hover) :deep(.ui-scroll-area__thumb) {
  background: var(--border-data);
}
</style>

<script setup lang="ts">
import { computed } from "vue";

interface Props {
  width?: string;
  height?: string;
}

const props = withDefaults(defineProps<Props>(), {
  width: undefined,
  height: undefined
});

const style = computed(() => ({
  width: props.width ?? "100%",
  height: props.height ?? "1em"
}));
</script>

<template>
  <div class="ui-skeleton" :style="style" aria-hidden="true" />
</template>

<style scoped>
/* Phase 1R §7.1 骨架屏：底 #FAFAF8，暖黑 4% 扫过，1600ms 循环 */
.ui-skeleton {
  border-radius: var(--radius-xs);
  background-color: var(--skeleton-base);
  background-image: linear-gradient(
    90deg,
    transparent,
    var(--skeleton-shimmer),
    transparent
  );
  background-size: 200% 100%;
  animation: ui-skeleton-shimmer 1600ms linear infinite;
}

@keyframes ui-skeleton-shimmer {
  from {
    background-position: 200% 0;
  }
  to {
    background-position: -200% 0;
  }
}
</style>

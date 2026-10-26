<script setup lang="ts">
interface Props {
  /** 表格标题：对读屏可见（Phase 2 §5.3 要求真 caption） */
  caption?: string;
}

withDefaults(defineProps<Props>(), { caption: undefined });
</script>

<template>
  <div class="ui-table__wrapper">
    <table class="ui-table">
      <caption v-if="caption" class="sr-only">
        {{
          caption
        }}
      </caption>
      <slot />
    </table>
  </div>
</template>

<style scoped>
.ui-table__wrapper {
  width: 100%;
  overflow-x: auto;
}

.ui-table {
  width: 100%;
  border-collapse: collapse;
  font-family: var(--font-ui);
  font-size: var(--font-size-body);
  line-height: var(--line-height-body);
  font-weight: var(--weight-regular);
  letter-spacing: var(--tracking-body);
  color: var(--text-primary);
}
</style>

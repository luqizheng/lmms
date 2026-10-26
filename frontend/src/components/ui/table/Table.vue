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
      <caption v-if="caption" class="ui-sr-only">
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

/* 读屏专用文本 —— 组件自带，不依赖尚未引入的全局 base.css */
.ui-sr-only {
  position: absolute;
  width: 1px;
  height: 1px;
  padding: 0;
  margin: -1px;
  overflow: hidden;
  clip: rect(0, 0, 0, 0);
  white-space: nowrap;
  border-width: 0;
}
</style>

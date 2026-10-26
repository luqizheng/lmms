<script setup lang="ts">
import { useI18n } from "vue-i18n";

const { t } = useI18n();
</script>

<template>
  <main class="ui-coming-soon">
    <h1 class="ui-coming-soon__title">
      {{ t("shell.status.comingSoonTitle") }}
    </h1>
    <p class="ui-coming-soon__hint">{{ t("shell.status.comingSoonHint") }}</p>
  </main>
</template>

<style scoped>
.ui-coming-soon {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: var(--space-3);
  min-height: 60vh;
  text-align: center;
}

.ui-coming-soon__title {
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-h3);
  line-height: var(--line-height-h3);
  font-weight: var(--weight-semibold);
  color: var(--text-primary);
}

.ui-coming-soon__hint {
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-small);
  color: var(--text-tertiary);
}
</style>

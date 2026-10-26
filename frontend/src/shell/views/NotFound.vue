<script setup lang="ts">
import { useI18n } from "vue-i18n";

const { t } = useI18n();
</script>

<template>
  <main class="ui-status-page">
    <span class="ui-status-page__code">404</span>
    <h1 class="ui-status-page__title">{{ t("shell.status.notFoundTitle") }}</h1>
    <p class="ui-status-page__hint">{{ t("shell.status.notFoundHint") }}</p>
    <RouterLink class="ui-status-page__back" to="/">{{
      t("shell.status.backHome")
    }}</RouterLink>
  </main>
</template>

<style scoped>
.ui-status-page {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: var(--space-3);
  min-height: 60vh;
  text-align: center;
}

.ui-status-page__code {
  font-family: var(--font-mono);
  font-size: var(--font-size-display);
  line-height: var(--line-height-display);
  font-variant-numeric: tabular-nums;
  color: var(--accent);
}

.ui-status-page__title {
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-h3);
  line-height: var(--line-height-h3);
  font-weight: var(--weight-semibold);
  color: var(--text-primary);
}

.ui-status-page__hint {
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-small);
  color: var(--text-tertiary);
}

.ui-status-page__back {
  color: var(--accent);
  text-decoration: none;
}

.ui-status-page__back:hover {
  color: var(--accent-hover);
}
</style>

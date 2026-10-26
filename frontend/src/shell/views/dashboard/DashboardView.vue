<script setup lang="ts">
import { Card } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { useI18n } from "vue-i18n";

const { t } = useI18n();

interface Metric {
  label: string;
  value: string;
  trend?: string;
  tone?: "neutral" | "success" | "warning" | "danger" | "info" | "accent";
}

const metrics: Metric[] = [
  { label: t("system.dashboard.activeUsers"), value: "12,480", trend: "+8.2%" },
  { label: t("system.dashboard.requests"), value: "1,284,930", trend: "+3.1%" },
  { label: t("system.dashboard.latency"), value: "36ms", tone: "accent" },
  {
    label: t("system.dashboard.errorRate"),
    value: "0.42%",
    trend: "-0.08%",
    tone: "success"
  }
];
</script>

<template>
  <section class="ui-dashboard">
    <header class="ui-dashboard__header">
      <h1 class="ui-dashboard__title">{{ t("system.dashboard.title") }}</h1>
    </header>

    <div class="ui-dashboard__metrics">
      <Card
        v-for="metric in metrics"
        :key="metric.label"
        class="ui-dashboard__metric"
      >
        <span class="ui-dashboard__metric-label">{{ metric.label }}</span>
        <span
          class="ui-dashboard__metric-value"
          :class="{ 'is-accent': metric.tone === 'accent' }"
        >
          {{ metric.value }}
        </span>
        <Badge v-if="metric.trend" :variant="metric.tone ?? 'neutral'">{{
          metric.trend
        }}</Badge>
      </Card>
    </div>
  </section>
</template>

<style scoped>
.ui-dashboard {
  display: flex;
  flex-direction: column;
  gap: var(--space-6);
}

.ui-dashboard__header {
  display: flex;
  align-items: center;
  justify-content: space-between;
}

.ui-dashboard__title {
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-h2);
  line-height: var(--line-height-h2);
  font-weight: var(--weight-semibold);
  letter-spacing: var(--tracking-h2);
  color: var(--text-primary);
}

/* 指标卡 4 → 2 → 1 列（Phase 2 §4.2） */
.ui-dashboard__metrics {
  display: grid;
  grid-template-columns: repeat(4, minmax(0, 1fr));
  gap: var(--space-4);
}

.ui-dashboard__metric {
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  gap: var(--space-2);
}

.ui-dashboard__metric-label {
  font-family: var(--font-ui);
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
  color: var(--text-tertiary);
}

.ui-dashboard__metric-value {
  font-family: var(--font-mono);
  font-size: var(--font-size-h2);
  line-height: var(--line-height-h2);
  font-variant-numeric: tabular-nums;
  color: var(--text-primary);
}

.ui-dashboard__metric-value.is-accent {
  color: var(--accent);
}

@media (max-width: 1024px) {
  .ui-dashboard__metrics {
    grid-template-columns: repeat(2, minmax(0, 1fr));
  }
}

@media (max-width: 640px) {
  .ui-dashboard__metrics {
    grid-template-columns: minmax(0, 1fr);
  }
}
</style>

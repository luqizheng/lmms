<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from "vue";

export type ToastVariant = "success" | "info" | "warning" | "danger";

interface Props {
  /** 语义变体 — Phase 2 §6.3 */
  variant?: ToastVariant;
  /** 标题 */
  title: string;
  /** 描述 */
  description?: string;
  /** 是否可关闭：成功 toast 必须可关 */
  closable?: boolean;
  /** 关闭按钮的无障碍名称 */
  closeLabel?: string;
  /** 停留时长（毫秒），默认 4000 */
  duration?: number;
  /** 是否打开（v-model:open） */
  open?: boolean;
}

const props = withDefaults(defineProps<Props>(), {
  variant: "info",
  description: undefined,
  closable: true,
  closeLabel: "关闭",
  duration: 4000,
  open: true
});

const emit = defineEmits<{ "update:open": [value: boolean] }>();

/** success / info 不打断当前朗读；warning / danger 立即播报（Phase 2 §5.3） */
const role = computed(() =>
  props.variant === "warning" || props.variant === "danger" ? "alert" : "status"
);

let timer: ReturnType<typeof setTimeout> | undefined;

function clearTimer() {
  if (timer !== undefined) {
    clearTimeout(timer);
    timer = undefined;
  }
}

function startTimer() {
  clearTimer();
  timer = setTimeout(() => {
    timer = undefined;
    emit("update:open", false);
  }, props.duration);
}

/** 悬停暂停计时（Phase 2 §6.3 / §7.1） */
function pause() {
  clearTimer();
}

onMounted(() => {
  if (props.open) startTimer();
});

onBeforeUnmount(() => {
  clearTimer();
});

function close() {
  clearTimer();
  emit("update:open", false);
}
</script>

<template>
  <div class="ui-toast__viewport" @pointermove="pause">
    <div class="ui-toast" :class="`ui-toast--${variant}`" :role="role">
      <span class="ui-toast__bar" aria-hidden="true" />
      <div class="ui-toast__content">
        <p class="ui-toast__title">{{ title }}</p>
        <p v-if="description" class="ui-toast__description">
          {{ description }}
        </p>
      </div>
      <button
        v-if="closable"
        type="button"
        class="ui-toast__close"
        :aria-label="closeLabel"
        @click="close"
      >
        <slot name="close">×</slot>
      </button>
    </div>
  </div>
</template>

<style scoped>
/* Toast — Phase 2 §6.3：白卡 + shadow-md + border-strong + 左侧 3px 语义色竖条 */
.ui-toast__viewport {
  position: fixed;
  right: var(--space-6);
  bottom: var(--space-6);
  z-index: 60;
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  pointer-events: none;
}

.ui-toast {
  position: relative;
  display: flex;
  align-items: flex-start;
  gap: var(--space-3);
  min-width: 280px;
  max-width: 420px;
  padding: var(--space-4);
  padding-left: var(--space-5);
  background: var(--surface-raised);
  border: 1px solid var(--border-strong);
  border-radius: var(--radius-md);
  box-shadow: var(--shadow-md);
  pointer-events: auto;
  animation: ui-toast-enter var(--duration-medium) var(--ease-decelerate);
}

.ui-toast__bar {
  position: absolute;
  left: 0;
  top: 0;
  bottom: 0;
  width: calc(var(--border-2) + var(--hairline));
  border-radius: var(--radius-md) 0 0 var(--radius-md);
}

.ui-toast--success .ui-toast__bar {
  background: var(--success);
}

.ui-toast--info .ui-toast__bar {
  background: var(--info);
}

.ui-toast--warning .ui-toast__bar {
  background: var(--warning);
}

.ui-toast--danger .ui-toast__bar {
  background: var(--danger);
}

.ui-toast__content {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
  flex: 1;
}

.ui-toast__title {
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-body);
  line-height: var(--line-height-body);
  font-weight: var(--weight-semibold);
  letter-spacing: var(--tracking-body);
  color: var(--text-primary);
}

.ui-toast__description {
  margin: 0;
  font-family: var(--font-ui);
  font-size: var(--font-size-small);
  line-height: var(--line-height-small);
  font-weight: var(--weight-regular);
  letter-spacing: var(--tracking-small);
  color: var(--text-secondary);
}

.ui-toast__close {
  flex: none;
  width: var(--space-6);
  height: var(--space-6);
  display: inline-flex;
  align-items: center;
  justify-content: center;
  padding: 0;
  border: 0;
  border-radius: var(--radius-xs);
  background: transparent;
  color: var(--text-secondary);
  font-size: var(--font-size-body);
  line-height: 1;
  cursor: pointer;
  transition:
    background-color var(--duration-fast) var(--ease-decelerate),
    color var(--duration-fast) var(--ease-decelerate);
}

.ui-toast__close:hover {
  background: var(--surface-hover-veil);
  color: var(--text-primary);
}

.ui-toast__close:focus-visible {
  outline: var(--size-focus-ring) solid var(--accent);
  outline-offset: var(--size-focus-ring);
}

@keyframes ui-toast-enter {
  from {
    opacity: 0;
    transform: translateY(var(--space-2));
  }
  to {
    opacity: 1;
    transform: translateY(0);
  }
}

/* 窄屏：顶部居中（Phase 2 §6.3） */
@media (max-width: 640px) {
  .ui-toast__viewport {
    right: 0;
    left: 0;
    bottom: auto;
    top: var(--space-4);
    align-items: center;
  }

  .ui-toast {
    max-width: calc(100% - var(--space-8));
  }
}

/* 减少动效 — Phase 2 §5.4：仅保留 opacity */
@media (prefers-reduced-motion: reduce) {
  .ui-toast {
    animation: none;
  }
}
</style>

<script setup lang="ts">
import { computed } from "vue";
import { Dialog } from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";
import { useI18n } from "vue-i18n";

interface Props {
  /** 是否打开（v-model:open） */
  open?: boolean;
  /** 弹框标题 */
  title?: string;
  /** 提交进行中 */
  submitting?: boolean;
  /** 提交按钮文案 */
  submitLabel?: string;
  /** 取消按钮文案 */
  cancelLabel?: string;
  /** 关闭按钮的无障碍名称 */
  closeLabel?: string;
  /** 弹框宽度档位 */
  size?: "sm" | "md" | "lg";
}

const { t } = useI18n();

const props = withDefaults(defineProps<Props>(), {
  open: false,
  title: undefined,
  submitting: false,
  submitLabel: undefined,
  cancelLabel: undefined,
  closeLabel: undefined,
  size: "md"
});

const emit = defineEmits<{
  "update:open": [value: boolean];
  submit: [];
  cancel: [];
}>();

const isOpen = computed({
  get: () => props.open,
  set: value => emit("update:open", value)
});
</script>

<template>
  <Dialog
    v-model:open="isOpen"
    :title="props.title"
    :size="props.size"
    :close-label="props.closeLabel ?? t('system.common.close')"
  >
    <form class="ui-form-dialog" novalidate @submit.prevent="emit('submit')">
      <div class="ui-form-dialog__body">
        <slot />
      </div>
      <footer class="ui-form-dialog__footer">
        <Button variant="secondary" type="button" @click="emit('cancel')">
          {{ props.cancelLabel ?? t("system.common.cancel") }}
        </Button>
        <Button type="submit" :loading="props.submitting">{{
          props.submitLabel
        }}</Button>
      </footer>
    </form>
  </Dialog>
</template>

<style scoped>
.ui-form-dialog {
  display: flex;
  flex-direction: column;
  margin: calc(var(--space-6) * -1);
}

.ui-form-dialog__body {
  display: flex;
  flex-direction: column;
  gap: var(--space-5);
  padding: var(--space-6);
}

.ui-form-dialog__footer {
  display: flex;
  align-items: center;
  justify-content: flex-end;
  gap: var(--space-3);
  padding: var(--space-4) var(--space-6);
  border-top: 1px solid var(--border-subtle);
}
</style>

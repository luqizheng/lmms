<script setup lang="ts">
import { computed, provide, ref } from "vue";
import { useI18n } from "vue-i18n";
import { Button } from "@/components/ui/button";
import { Dialog } from "@/components/ui/dialog";
import {
  CONFIRM_HOST_KEY,
  type ConfirmApi,
  type ConfirmOptions
} from "../composables/useConfirm";

const { t } = useI18n();

const open = ref(false);
const options = ref<ConfirmOptions | null>(null);
let resolver: ((result: boolean) => void) | null = null;

function confirm(input: ConfirmOptions): Promise<boolean> {
  options.value = input;
  open.value = true;
  return new Promise<boolean>(resolve => {
    resolver = resolve;
  });
}

function settle(result: boolean) {
  open.value = false;
  options.value = null;
  resolver?.(result);
  resolver = null;
}

provide(CONFIRM_HOST_KEY, { confirm } satisfies ConfirmApi);

const title = computed(() => options.value?.title ?? "");
const description = computed(() => options.value?.description ?? "");
</script>

<template>
  <Dialog
    :open="open"
    size="sm"
    :title="title"
    :close-label="options?.cancelLabel ?? t('common.cancel')"
    @update:open="value => { if (!value) settle(false); }"
  >
    <p v-if="description" class="ui-confirm__description">{{ description }}</p>
    <template #footer>
      <Button variant="secondary" @click="settle(false)">
        {{ options?.cancelLabel ?? t("system.common.cancel") }}
      </Button>
      <Button
        :class="{ 'ui-confirm__danger': options?.danger }"
        @click="settle(true)"
      >
        {{ options?.confirmLabel ?? t("system.common.confirm") }}
      </Button>
    </template>
  </Dialog>
</template>

<style scoped>
.ui-confirm__description {
  margin: 0 0 var(--space-4);
  font-family: var(--font-ui);
  font-size: var(--font-size-body);
  line-height: var(--line-height-body);
  color: var(--text-secondary);
}

.ui-confirm__danger {
  background: var(--danger);
  color: var(--danger-contrast-text, #ffffff);
}

.ui-confirm__danger:hover {
  background: var(--danger-strong);
}
</style>

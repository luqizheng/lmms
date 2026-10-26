<script setup lang="ts">
import { provide, ref } from "vue";
import Toast, { type ToastVariant } from "./Toast.vue";
import { TOAST_HOST_KEY, type ToastApi, type ToastOptions } from "./useToast";


/**
 * Toast 宿主：同屏最多 1 条，新 toast 替换旧 toast（Phase 2 §6.3）。
 *
 * 用法：应用挂载一个 <ToastHost />，业务代码通过模板 ref 调用
 * `host.value.toast({ variant: "success", title: "已保存" })`。
 */
const current = ref<ToastOptions | null>(null);
const seq = ref(0);

function toast(options: ToastOptions) {
  current.value = { ...options };
  seq.value += 1;
}

function dismiss() {
  current.value = null;
}

defineExpose({ toast, dismiss });

// 让页面通过 useToast() 拿到同一个宿主
provide(TOAST_HOST_KEY, { toast, dismiss } satisfies ToastApi);
</script>

<template>
  <div class="ui-toast-host">
    <Toast v-if="current" :key="seq" v-bind="current" @update:open="dismiss" />
  </div>
</template>

<style scoped>
.ui-toast-host {
  position: relative;
  z-index: 60;
}
</style>

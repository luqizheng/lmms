<template>
  <span v-if="loading" class="re-user-label re-user-label--loading">
    <span class="re-user-label__skeleton" />
  </span>
  <span v-else class="re-user-label">
    {{ nickname }}
  </span>
</template>

<script setup lang="ts">
import { ref, onMounted, watch } from "vue";
import { getUserNickname } from "./cache";

const props = defineProps<{
  /** 用户 ID */
  userId: string;
}>();

const nickname = ref(props.userId);
const loading = ref(true);

async function load(): Promise<void> {
  if (!props.userId) {
    loading.value = false;
    nickname.value = "";
    return;
  }
  loading.value = true;
  nickname.value = await getUserNickname(props.userId);
  loading.value = false;
}

onMounted(() => {
  load();
});

watch(
  () => props.userId,
  () => {
    load();
  }
);
</script>

<style scoped>
.re-user-label {
  display: inline;
}

.re-user-label--loading {
  vertical-align: middle;
}

.re-user-label__skeleton {
  display: inline-block;
  width: 3em;
  height: 1em;
  border-radius: 3px;
  background: linear-gradient(
    90deg,
    var(--el-color-info-light-7, #e8e9eb) 25%,
    var(--el-color-info-light-9, #f4f4f5) 37%,
    var(--el-color-info-light-7, #e8e9eb) 63%
  );
  background-size: 400% 100%;
  animation: re-user-label-shimmer 1.4s ease infinite;
  vertical-align: middle;
}

@keyframes re-user-label-shimmer {
  0% {
    background-position: 100% 50%;
  }
  100% {
    background-position: 0 50%;
  }
}
</style>

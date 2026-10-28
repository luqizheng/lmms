<script setup lang="ts">
import { computed, onMounted, watch } from "vue";
import { useI18n } from "vue-i18n";
import { useUserDirectory, shortUserId } from "../store/user-directory";

interface Props {
  /** 用户 ID（负责人 / 创建人 / 操作人等） */
  userId?: string | null;
  /** 未解析到名字时的兜底文案；缺省走 i18n `common.user` */
  fallback?: string;
}

const props = defineProps<Props>();
const { t } = useI18n();
const directory = useUserDirectory();

const display = computed(() => {
  const name = directory.nameOf(props.userId);
  if (name) return name;
  if (!props.userId) return "—";
  return props.fallback ?? t("common.user", { id: shortUserId(props.userId) });
});

function load() {
  if (props.userId) void directory.ensureLoaded();
}

onMounted(load);
watch(() => props.userId, load);
</script>

<template>
  <span class="ui-user-label">{{ display }}</span>
</template>

<style scoped>
.ui-user-label {
  display: inline-block;
  max-width: 100%;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  vertical-align: bottom;
  color: inherit;
}
</style>

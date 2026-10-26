<script setup lang="ts">
import { computed } from "vue";
import { AvatarFallback, AvatarImage, AvatarRoot } from "reka-ui";

interface Props {
  /** 图片地址；为空时直接渲染回退 */
  src?: string;
  /** 图片替代文本 */
  alt?: string;
  /** 用户名称：用于派生首字母回退 */
  name?: string;
  /** 头像的无障碍名称（头像独立出现、旁边没有姓名文本时使用） */
  label?: string;
}

const props = withDefaults(defineProps<Props>(), {
  src: undefined,
  alt: undefined,
  name: undefined,
  label: undefined
});

/** 首字母回退 — Phase 1R §6.9：侧边栏底部用户区头像 */
const initial = computed(() => {
  const source = (props.name ?? props.alt ?? "").trim();
  return source ? source.charAt(0).toUpperCase() : "";
});

/** 有无障碍名称时，图片本身退化为装饰，避免同一信息被朗读两次 */
const imageAlt = computed(() =>
  props.label ? "" : (props.alt ?? props.name ?? "")
);
</script>

<template>
  <AvatarRoot class="ui-avatar" :aria-label="label">
    <AvatarImage
      v-if="src"
      class="ui-avatar__image"
      :src="src"
      :alt="imageAlt"
    />
    <AvatarFallback
      class="ui-avatar__fallback"
      :aria-hidden="label ? 'true' : undefined"
    >
      <slot name="fallback">{{ initial }}</slot>
    </AvatarFallback>
  </AvatarRoot>
</template>

<style scoped>
/* 头像 — Phase 1R §6.9：直径 = --space-6 + --space-1 的圆 */
.ui-avatar {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  flex: none;
  box-sizing: border-box;
  /* 直径 = --space-6(24) + --space-1(4) */
  width: calc(var(--space-6) + var(--space-1));
  height: calc(var(--space-6) + var(--space-1));
  border-radius: var(--radius-full);
  overflow: hidden;
  background: var(--surface-sunken);
  font-family: var(--font-ui);
  user-select: none;
}

.ui-avatar__image {
  width: 100%;
  height: 100%;
  object-fit: cover;
}

/* 回退 — 底 --surface-sunken / 字 --text-secondary */
.ui-avatar__fallback {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 100%;
  height: 100%;
  background: var(--surface-sunken);
  color: var(--text-secondary);
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
  font-weight: var(--weight-semibold);
  letter-spacing: var(--tracking-caption);
}
</style>

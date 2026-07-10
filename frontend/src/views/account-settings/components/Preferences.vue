<script setup lang="ts">
import { ref } from "vue";
import { useI18n } from "vue-i18n";
import { message } from "@/utils/message";
import { deviceDetection } from "@pureadmin/utils";

defineOptions({
  name: "Preferences"
});

const { t } = useI18n();

const list = ref([
  {
    title: t("menus.accountCardTitlePassword"),
    illustrate: t("menus.accountCardIllustratePassword"),
    checked: true
  },
  {
    title: t("menus.accountCardTitleSystemMessage"),
    illustrate: t("menus.accountCardIllustrateSystemMessage"),
    checked: true
  },
  {
    title: t("menus.accountCardTitleTodoTask"),
    illustrate: t("menus.accountCardIllustrateTodoTask"),
    checked: true
  }
]);

function onChange(val, item) {
  console.log("onChange", val);
  message(t("menus.accountCardSetSuccess", { title: item.title }), {
    type: "success"
  });
}
</script>

<template>
  <div :class="['min-w-45', deviceDetection() ? 'max-w-full' : 'max-w-[70%]']">
    <h3 class="my-8!">{{ t("menus.purePreferences") }}</h3>
    <div v-for="(item, index) in list" :key="index">
      <div class="flex items-center">
        <div class="flex-1">
          <p>{{ item.title }}</p>
          <p class="wp-4">
            <el-text class="mx-1" type="info">
              {{ item.illustrate }}
            </el-text>
          </p>
        </div>
        <el-switch
          v-model="item.checked"
          inline-prompt
          :active-text="t('menus.accountCardSwitchActive')"
          :inactive-text="t('menus.accountCardSwitchInactive')"
          @change="val => onChange(val, item)"
        />
      </div>
      <el-divider />
    </div>
  </div>
</template>

<style lang="scss" scoped>
.el-divider--horizontal {
  border-top: 0.1px var(--el-border-color) var(--el-border-style);
}
</style>
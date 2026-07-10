<script setup lang="ts">
import { message } from "@/utils/message";
import { useI18n } from "vue-i18n";
import { initRouter } from "@/router/utils";
import { storageLocal } from "@pureadmin/utils";
import { type CSSProperties, ref, computed } from "vue";
import { useUserStoreHook } from "@/store/modules/user";
import { usePermissionStoreHook } from "@/store/modules/permission";

defineOptions({
  name: "PermissionPage"
});
const { t } = useI18n();

const elStyle = computed((): CSSProperties => {
  return {
    width: "85vw",
    justifyContent: "start"
  };
});

const username = ref(useUserStoreHook()?.username);

const options = computed(() => [
  { value: "admin", label: t("menus.permsDemoRoleAdmin") },
  { value: "common", label: t("menus.permsDemoRoleUser") }
]);

function onChange() {
  useUserStoreHook()
    .loginByUsername({ username: username.value, password: "admin123" })
    .then(() => {
      storageLocal().removeItem("async-routes");
      usePermissionStoreHook().clearAllCachePage();
      initRouter();
    })
    .catch(err => {
      message(err, { type: "error" });
    });
}
</script>

<template>
  <div>
    <p class="mb-2!">{{ t("menus.permsDemoRoleSimulate") }}</p>
    <el-card shadow="never" :style="elStyle">
      <template #header>
        <div class="card-header">
          <span>{{ t("menus.permsDemoCurrentRole", { name: username }) }}</span>
        </div>
        <el-link
          class="mt-2"
          href="https://github.com/pure-admin/vue-pure-admin/blob/main/src/views/permission/page/index.vue"
          target="_blank"
        >
          {{ t("menus.permsDemoCodeLine", { path: "src/views/permission/page/index.vue" }) }}
        </el-link>
      </template>
      <el-select v-model="username" class="w-40!" @change="onChange">
        <el-option
          v-for="item in options"
          :key="item.value"
          :label="item.label"
          :value="item.value"
        />
      </el-select>
    </el-card>
  </div>
</template>
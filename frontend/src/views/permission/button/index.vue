<script setup lang="ts">
import { useI18n } from "vue-i18n";
import { hasAuth, getAuths } from "@/router/utils";

defineOptions({
  name: "PermissionButtonRouter"
});
const { t } = useI18n();
</script>

<template>
  <div>
    <p class="mb-2!">
      {{ t("menus.permsDemoCurrentAuths") }}：{{ getAuths() }}
    </p>

    <el-card shadow="never" class="mb-2">
      <template #header>
        <div class="card-header">{{ t("menus.permsDemoTitle1") }}</div>
        <el-link
          class="mt-2"
          href="https://github.com/pure-admin/vue-pure-admin/blob/main/src/views/permission/button/index.vue"
          target="_blank"
        >
          {{
            t("menus.permsDemoCodeLine", {
              path: "src/views/permission/button/index.vue"
            })
          }}
        </el-link>
      </template>
      <el-space wrap>
        <Auth :value="t('menus.permsDemoCodeList1')">
          <el-button plain type="warning">
            {{
              t("menus.permsDemoHaveCode", {
                code: t("menus.permsDemoCodeList1")
              })
            }}
          </el-button>
        </Auth>
        <Auth :value="[t('menus.permsDemoCodeList2')]">
          <el-button plain type="primary">
            {{
              t("menus.permsDemoHaveCode", {
                code: `[${t("menus.permsDemoCodeList2")}]`
              })
            }}
          </el-button>
        </Auth>
        <Auth
          :value="[
            t('menus.permsDemoCodeList1'),
            t('menus.permsDemoCodeList2'),
            t('menus.permsDemoCodeList3Delete')
          ]"
        >
          <el-button plain type="danger">
            {{
              t("menus.permsDemoHasCodes", {
                codes: `['${t("menus.permsDemoCodeList1")}', '${t("menus.permsDemoCodeList2")}', '${t("menus.permsDemoCodeList3Delete")}']`
              })
            }}
          </el-button>
        </Auth>
      </el-space>
    </el-card>

    <el-card shadow="never" class="mb-2">
      <template #header>
        <div class="card-header">{{ t("menus.permsDemoTitle2") }}</div>
      </template>
      <el-space wrap>
        <el-button
          v-if="hasAuth(t('menus.permsDemoCodeList1'))"
          plain
          type="warning"
        >
          {{
            t("menus.permsDemoHaveCode", {
              code: t("menus.permsDemoCodeList1")
            })
          }}
        </el-button>
        <el-button
          v-if="hasAuth([t('menus.permsDemoCodeList2')])"
          plain
          type="primary"
        >
          {{
            t("menus.permsDemoHaveCode", {
              code: `[${t("menus.permsDemoCodeList2")}]`
            })
          }}
        </el-button>
        <el-button
          v-if="
            hasAuth([
              t('menus.permsDemoCodeList1'),
              t('menus.permsDemoCodeList2'),
              t('menus.permsDemoCodeList3Delete')
            ])
          "
          plain
          type="danger"
        >
          {{
            t("menus.permsDemoHasCodes", {
              codes: `['${t("menus.permsDemoCodeList1")}', '${t("menus.permsDemoCodeList2")}', '${t("menus.permsDemoCodeList3Delete")}']`
            })
          }}
        </el-button>
      </el-space>
    </el-card>

    <el-card shadow="never">
      <template #header>
        <div class="card-header">
          {{ t("menus.permsDemoTitle3") + t("menus.permsDemoTitle3Note") }}
        </div>
      </template>
      <el-space wrap>
        <el-button v-auth="t('menus.permsDemoCodeList1')" plain type="warning">
          {{
            t("menus.permsDemoHaveCode", {
              code: t("menus.permsDemoCodeList1")
            })
          }}
        </el-button>
        <el-button
          v-auth="[t('menus.permsDemoCodeList2')]"
          plain
          type="primary"
        >
          {{
            t("menus.permsDemoHaveCode", {
              code: `[${t("menus.permsDemoCodeList2")}]`
            })
          }}
        </el-button>
        <el-button
          v-auth="[
            t('menus.permsDemoCodeList1'),
            t('menus.permsDemoCodeList2'),
            t('menus.permsDemoCodeList3Delete')
          ]"
          plain
          type="danger"
        >
          {{
            t("menus.permsDemoHasCodes", {
              codes: `['${t("menus.permsDemoCodeList1")}', '${t("menus.permsDemoCodeList2")}', '${t("menus.permsDemoCodeList3Delete")}']`
            })
          }}
        </el-button>
      </el-space>
    </el-card>
  </div>
</template>

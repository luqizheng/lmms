<script setup lang="ts">
import dayjs from "dayjs";
import { getMineLogs } from "@/api/user";
import { reactive, ref, onMounted } from "vue";
import { useI18n } from "vue-i18n";
import { deviceDetection } from "@pureadmin/utils";
import type { PaginationProps } from "@pureadmin/table";

defineOptions({
  name: "SecurityLog"
});

const { t } = useI18n();

const loading = ref(true);
const dataList = ref([]);
const pagination = reactive<PaginationProps>({
  total: 0,
  pageSize: 10,
  currentPage: 1,
  background: true,
  layout: "prev, pager, next"
});
const columns: TableColumnList = [
  {
    label: t("menus.securityLogDetail"),
    prop: "summary",
    minWidth: 140
  },
  {
    label: t("menus.securityLogIp"),
    prop: "ip",
    minWidth: 100
  },
  {
    label: t("menus.securityLogLocation"),
    prop: "address",
    minWidth: 140
  },
  {
    label: t("menus.securityLogOs"),
    prop: "system",
    minWidth: 100
  },
  {
    label: t("menus.securityLogBrowser"),
    prop: "browser",
    minWidth: 100
  },
  {
    label: t("menus.securityLogTime"),
    prop: "operatingTime",
    minWidth: 180,
    formatter: ({ operatingTime }) =>
      dayjs(operatingTime).format("YYYY-MM-DD HH:mm:ss")
  }
];

async function onSearch() {
  loading.value = true;
  const { code, data } = await getMineLogs();
  if (code === 0) {
    dataList.value = data.list;
    pagination.total = data.total;
    pagination.pageSize = data.pageSize;
    pagination.currentPage = data.currentPage;
  }

  setTimeout(() => {
    loading.value = false;
  }, 200);
}

onMounted(() => {
  onSearch();
});
</script>

<template>
  <div :class="['min-w-45', deviceDetection() ? 'max-w-full' : 'max-w-[70%]']">
    <h3 class="my-8!">{{ t("menus.pureSecurityLog") }}</h3>
    <pure-table
      row-key="id"
      table-layout="auto"
      :loading="loading"
      :data="dataList"
      :columns="columns"
      :pagination="pagination"
    />
  </div>
</template>

<script setup lang="ts">
import { ref } from "vue";
import { useInbox } from "./utils/hook";
import { PureTableBar } from "@/components/RePureTableBar";
import { useRenderIcon } from "@/components/ReIcon/src/hooks";
import Refresh from "~icons/ep/refresh";
import Search from "~icons/ri/search-line";

defineOptions({ name: "NotificationInbox" });
const formRef = ref();
const { form, loading, columns, dataList, onSearch, openDetail } = useInbox();

function reset() {
  formRef.value?.resetFields();
  onSearch();
}
</script>

<template>
  <div class="main">
    <el-form
      ref="formRef"
      :inline="true"
      :model="form"
      class="search-form bg-bg_color w-full pl-8 pt-3 overflow-auto"
    >
      <el-form-item label="标题：" prop="keyword">
        <el-input
          v-model="form.keyword"
          placeholder="按标题筛选"
          clearable
          class="w-50!"
        />
      </el-form-item>
      <el-form-item>
        <el-button
          type="primary"
          :icon="useRenderIcon(Search)"
          :loading="loading"
          @click="onSearch"
        >
          搜索
        </el-button>
        <el-button :icon="useRenderIcon(Refresh)" @click="reset">
          重置
        </el-button>
      </el-form-item>
    </el-form>
    <PureTableBar title="通知收件箱" @refresh="onSearch">
      <template v-slot="{ size, dynamicColumns }">
        <pure-table
          align-whole="center"
          :loading="loading"
          :size="size"
          :data="dataList"
          :columns="dynamicColumns"
          @row-click="openDetail"
        />
      </template>
    </PureTableBar>
  </div>
</template>

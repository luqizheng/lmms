<script setup lang="ts">
import { ref } from "vue";
import { useAdmin } from "./utils/hook";
import { PureTableBar } from "@/components/RePureTableBar";
import { useRenderIcon } from "@/components/ReIcon/src/hooks";
import AddFill from "~icons/ri/add-circle-line";
import EditPen from "~icons/ep/edit-pen";
import VideoPlay from "~icons/ep/video-play";
import Refresh from "~icons/ep/refresh";

defineOptions({ name: "NotificationAdmin" });
const { form, loading, columns, dataList, onSearch, openDialog, handlePublish, handleRevoke } =
  useAdmin();
const formRef = ref();

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
      <el-form-item label="状态：" prop="status">
        <el-select
          v-model="form.status"
          placeholder="全部"
          clearable
          class="w-40!"
        >
          <el-option label="draft" value="draft" />
          <el-option label="published" value="published" />
          <el-option label="revoked" value="revoked" />
        </el-select>
      </el-form-item>
      <el-form-item>
        <el-button
          type="primary"
          :icon="useRenderIcon('ri/search-line')"
          :loading="loading"
          @click="onSearch"
        >
          搜索
        </el-button>
        <el-button :icon="useRenderIcon(Refresh)" @click="reset">重置</el-button>
      </el-form-item>
    </el-form>

    <PureTableBar title="公告管理" @refresh="onSearch">
      <template #buttons>
        <el-button
          type="primary"
          :icon="useRenderIcon(AddFill)"
          @click="openDialog()"
        >
          新建公告
        </el-button>
      </template>
      <template v-slot="{ size, dynamicColumns }">
        <pure-table
          align-whole="center"
          :loading="loading"
          :size="size"
          :data="dataList"
          :columns="dynamicColumns"
        >
          <template #operation="{ row }">
            <el-button
              link
              type="primary"
              :icon="useRenderIcon(EditPen)"
              @click="openDialog(row)"
            >
              编辑
            </el-button>
            <el-button
              link
              type="success"
              :icon="useRenderIcon(VideoPlay)"
              @click="handlePublish(row)"
            >
              发布
            </el-button>
            <el-button link type="danger" @click="handleRevoke(row)">
              撤销
            </el-button>
          </template>
        </pure-table>
      </template>
    </PureTableBar>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted } from "vue";
import { listSpaces, createSpace, type KbSpace } from "@/api/kb";

defineOptions({ name: "PmKbSpaces" });

const loading = ref(false);
const dataList = ref<KbSpace[]>([]);
const total = ref(0);
const page = ref(1);
const size = ref(20);

const dialogVisible = ref(false);
const form = ref<{ name: string; description: string; visibility: "private" | "project" | "open" }>({
  name: "",
  description: "",
  visibility: "project"
});

async function fetchData() {
  loading.value = true;
  try {
    const { data } = await listSpaces({ page: page.value, size: size.value });
    dataList.value = data.items;
    total.value = data.total;
  } finally {
    loading.value = false;
  }
}

function onSizeChange(v: number) {
  size.value = v;
  fetchData();
}
function onPageChange(v: number) {
  page.value = v;
  fetchData();
}

function openCreate() {
  form.value = { name: "", description: "", visibility: "project" };
  dialogVisible.value = true;
}

async function submitCreate() {
  if (!form.value.name) return;
  await createSpace(form.value);
  dialogVisible.value = false;
  fetchData();
}

onMounted(fetchData);
</script>

<template>
  <div class="pm-kb-spaces">
    <div class="toolbar">
      <el-button type="primary" @click="openCreate">新建空间</el-button>
    </div>

    <el-table v-loading="loading" :data="dataList" stripe>
      <el-table-column prop="id" label="ID" width="80" />
      <el-table-column prop="name" label="名称" min-width="160" />
      <el-table-column prop="description" label="描述" min-width="240" show-overflow-tooltip />
      <el-table-column prop="visibility" label="可见性" width="100">
        <template #default="{ row }">
          <el-tag size="small">{{ row?.visibility }}</el-tag>
        </template>
      </el-table-column>
      <el-table-column prop="createdAt" label="创建于" width="180" />
      <el-table-column label="操作" width="120" fixed="right">
        <template #default="{ row }">
          <el-button size="small" link disabled>文档</el-button>
        </template>
      </el-table-column>
    </el-table>

    <el-pagination
      v-model:current-page="page"
      v-model:page-size="size"
      :total="total"
      layout="total, sizes, prev, pager, next, jumper"
      :page-sizes="[10, 20, 50, 100]"
      @size-change="onSizeChange"
      @current-change="onPageChange"
    />

    <el-dialog v-model="dialogVisible" title="新建知识库空间" width="480">
      <el-form label-width="80px">
        <el-form-item label="名称" required>
          <el-input v-model="form.name" placeholder="空间名称" />
        </el-form-item>
        <el-form-item label="描述">
          <el-input v-model="form.description" type="textarea" :rows="3" />
        </el-form-item>
        <el-form-item label="可见性">
          <el-radio-group v-model="form.visibility">
            <el-radio-button value="private">私有</el-radio-button>
            <el-radio-button value="project">项目内</el-radio-button>
            <el-radio-button value="open">公开</el-radio-button>
          </el-radio-group>
        </el-form-item>
      </el-form>
      <template #footer>
        <el-button @click="dialogVisible = false">取消</el-button>
        <el-button type="primary" @click="submitCreate">创建</el-button>
      </template>
    </el-dialog>
  </div>
</template>

<style scoped>
.pm-kb-spaces {
  padding: 16px;
}
.toolbar {
  margin-bottom: 16px;
}
.el-pagination {
  margin-top: 16px;
  justify-content: flex-end;
}
</style>

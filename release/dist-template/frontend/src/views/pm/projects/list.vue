<script setup lang="ts">
import { ref, onMounted } from "vue";
import { useRouter } from "vue-router";
import {
  listProjects,
  createProject,
  archiveProject,
  type Project,
  type Paged
} from "@/api/pm";

defineOptions({ name: "PmProjectList" });

const router = useRouter();
const loading = ref(false);
const dataList = ref<Project[]>([]);
const total = ref(0);
const page = ref(1);
const size = ref(20);
const filterStatus = ref<string>("");

const dialogVisible = ref(false);
const form = ref<{ code: string; name: string; description: string }>({
  code: "",
  name: "",
  description: ""
});

const statusTagType: Record<string, "primary" | "info" | "success" | "warning" | "danger"> = {
  active: "success",
  archived: "info"
};

const statusLabel: Record<string, string> = {
  active: "活跃",
  archived: "已归档"
};

async function fetchData() {
  loading.value = true;
  try {
    const params: { page: number; size: number; status?: string } = {
      page: page.value,
      size: size.value
    };
    if (filterStatus.value) params.status = filterStatus.value;
    const { data }: { data: Paged<Project> } = await listProjects(params);
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
  form.value = { code: "", name: "", description: "" };
  dialogVisible.value = true;
}

async function submitCreate() {
  if (!form.value.code || !form.value.name) return;
  await createProject(form.value);
  dialogVisible.value = false;
  fetchData();
}

function goDetail(row: Project) {
  router.push({ name: "PmProjectDetail", params: { id: row.id } });
}

async function archive(row: Project) {
  await archiveProject(row.id);
  fetchData();
}

onMounted(fetchData);
</script>

<template>
  <div class="pm-projects">
    <div class="toolbar">
      <el-input
        v-model="filterStatus"
        placeholder="按状态过滤 (active/archived)"
        clearable
        style="width: 240px"
        @clear="fetchData"
        @keyup.enter="fetchData"
      />
      <el-button @click="fetchData" style="margin-left: 8px">查询</el-button>
      <el-button type="primary" @click="openCreate" style="margin-left: 8px">
        新建项目
      </el-button>
    </div>

    <el-table v-loading="loading" :data="dataList" stripe @row-click="goDetail">
      <el-table-column prop="id" label="ID" width="80" />
      <el-table-column prop="code" label="项目代号" width="120" />
      <el-table-column prop="name" label="项目名" min-width="180" />
      <el-table-column prop="description" label="描述" min-width="240" show-overflow-tooltip />
      <el-table-column label="状态" width="100">
        <template #default="{ row }">
          <el-tag :type="statusTagType[row?.status]" size="small">
            {{ statusLabel[row?.status] ?? row?.status }}
          </el-tag>
        </template>
      </el-table-column>
      <el-table-column prop="ownerId" label="负责人ID" width="100" />
      <el-table-column prop="updatedAt" label="最近更新" width="180" />
      <el-table-column label="操作" width="160" fixed="right">
        <template #default="{ row }">
          <el-button size="small" link @click.stop="goDetail(row)">详情</el-button>
          <el-button
            v-if="row?.status === 'active'"
            size="small"
            link
            type="danger"
            @click.stop="archive(row)"
          >
            归档
          </el-button>
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

    <el-dialog v-model="dialogVisible" title="新建项目" width="480">
      <el-form label-width="80px">
        <el-form-item label="项目代号" required>
          <el-input v-model="form.code" placeholder="英文短代码，如 ACME" />
        </el-form-item>
        <el-form-item label="项目名" required>
          <el-input v-model="form.name" />
        </el-form-item>
        <el-form-item label="描述">
          <el-input v-model="form.description" type="textarea" :rows="3" />
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
.pm-projects {
  padding: 16px;
}
.toolbar {
  margin-bottom: 16px;
  display: flex;
  align-items: center;
}
.el-pagination {
  margin-top: 16px;
  justify-content: flex-end;
}
</style>

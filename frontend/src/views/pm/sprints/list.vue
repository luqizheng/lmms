<script setup lang="ts">
import { ref, onMounted, computed } from "vue";
import { useRoute, useRouter } from "vue-router";
import { ElMessage } from "element-plus";
import {
  listSprints,
  createSprint,
  updateSprint,
  deleteSprint,
  getProject,
  type Sprint,
  type Project
} from "@/api/pm";

defineOptions({ name: "PmSprints" });

const route = useRoute();
const router = useRouter();
const projectId = ref<number>(Number(route.params.projectId));

const project = ref<Project | null>(null);
const loading = ref(false);
const dataList = ref<Sprint[]>([]);
const total = ref(0);
const page = ref(1);
const size = ref(20);
const statusFilter = ref<string>("");

const dialogVisible = ref(false);
const form = ref({ name: "", goal: "", startDate: "", endDate: "", status: "planning" });

const statusTagType = computed(() => (v: string) =>
  ({ planning: "info", active: "success", completed: "default" })[v] ?? "info");

async function fetchData() {
  loading.value = true;
  try {
    const p: Record<string, unknown> = { page: page.value, size: size.value };
    if (statusFilter.value) p.status = statusFilter.value;
    const { data } = await listSprints(projectId.value, p);
    dataList.value = data.items;
    total.value = data.total;
  } finally { loading.value = false; }
}

async function fetchProject() {
  try {
    const { data } = await getProject(projectId.value);
    project.value = data;
  } catch {}
}

function onSizeChange(v: number) { size.value = v; fetchData(); }
function onPageChange(v: number) { page.value = v; fetchData(); }

function openCreate() {
  form.value = { name: "", goal: "", startDate: "", endDate: "", status: "planning" };
  dialogVisible.value = true;
}

async function submitCreate() {
  if (!form.value.name) { ElMessage.warning("名称不能为空"); return; }
  const payload: Record<string, unknown> = { name: form.value.name, status: form.value.status };
  if (form.value.goal) payload.goal = form.value.goal;
  if (form.value.startDate) payload.startDate = form.value.startDate;
  if (form.value.endDate) payload.endDate = form.value.endDate;
  await createSprint(projectId.value, payload as any);
  ElMessage.success("迭代创建成功");
  dialogVisible.value = false;
  await fetchData();
}

async function changeStatus(row: Sprint, newStatus: string) {
  if (row.status === newStatus) return;
  await updateSprint(row.id, { status: newStatus as Sprint["status"] });
  ElMessage.success("状态已更新");
  await fetchData();
}

async function removeSprint(row: Sprint) {
  await deleteSprint(row.id);
  ElMessage.success("已删除");
  await fetchData();
}

onMounted(async () => {
  await fetchProject();
  await fetchData();
});
</script>

<template>
  <div class="pm-sprints">
    <el-page-header @back="router.back()">
      <template #content>
        <span>{{ project?.name ?? "项目" }} - 迭代</span>
      </template>
      <template #extra>
        <el-button type="primary" @click="openCreate">新建迭代</el-button>
      </template>
    </el-page-header>

    <div class="filters">
      <el-select
        v-model="statusFilter" placeholder="按状态筛选" clearable
        style="width: 160px" @change="fetchData"
      >
        <el-option label="规划中" value="planning" />
        <el-option label="进行中" value="active" />
        <el-option label="已完成" value="completed" />
      </el-select>
    </div>

    <div v-loading="loading" style="margin-top: 16px">
      <el-empty v-if="dataList.length === 0" description="还没有迭代" />
      <el-table v-else :data="dataList" stripe>
        <el-table-column prop="id" label="ID" width="60" />
        <el-table-column prop="name" label="名称" min-width="160" />
        <el-table-column prop="goal" label="目标" min-width="200" show-overflow-tooltip />
        <el-table-column label="状态" width="120">
          <template #default="{ row }">
            <el-select :model-value="row.status" size="small" style="width: 100px"
              @change="(v: string) => changeStatus(row, v)">
              <el-option label="规划中" value="planning" />
              <el-option label="进行中" value="active" />
              <el-option label="已完成" value="completed" />
            </el-select>
          </template>
        </el-table-column>
        <el-table-column prop="startDate" label="开始日期" width="120" />
        <el-table-column prop="endDate" label="结束日期" width="120" />
        <el-table-column prop="createdAt" label="创建于" width="180" />
        <el-table-column label="操作" width="80" fixed="right">
          <template #default="{ row }">
            <el-button size="small" link type="danger" @click="removeSprint(row)">删除</el-button>
          </template>
        </el-table-column>
      </el-table>

      <el-pagination
        v-model:current-page="page" v-model:page-size="size"
        :total="total" layout="total, sizes, prev, pager, next, jumper"
        :page-sizes="[10, 20, 50, 100]"
        @size-change="onSizeChange" @current-change="onPageChange"
      />
    </div>

    <el-dialog v-model="dialogVisible" title="新建迭代" width="520">
      <el-form label-width="80px">
        <el-form-item label="名称" required>
          <el-input v-model="form.name" placeholder="迭代名称" />
        </el-form-item>
        <el-form-item label="目标">
          <el-input v-model="form.goal" type="textarea" :rows="2" />
        </el-form-item>
        <el-form-item label="状态">
          <el-select v-model="form.status" style="width: 100%">
            <el-option label="规划中" value="planning" />
            <el-option label="进行中" value="active" />
            <el-option label="已完成" value="completed" />
          </el-select>
        </el-form-item>
        <el-form-item label="开始日期">
          <el-input v-model="form.startDate" placeholder="YYYY-MM-DD" />
        </el-form-item>
        <el-form-item label="结束日期">
          <el-input v-model="form.endDate" placeholder="YYYY-MM-DD" />
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
.pm-sprints { padding: 16px; }
.filters { margin-top: 16px; }
.el-pagination { margin-top: 16px; justify-content: flex-end; }
</style>

<script setup lang="ts">
import { ref, onMounted, computed } from "vue";
import { useRoute, useRouter } from "vue-router";
import { ElMessage } from "element-plus";
import {
  listTasks,
  createTask,
  updateTask,
  deleteTask,
  getProject,
  type Task,
  type Project
} from "@/api/pm";

defineOptions({ name: "PmTasks" });

const route = useRoute();
const router = useRouter();
const projectId = ref<number>(Number(route.params.projectId));

const project = ref<Project | null>(null);
const loading = ref(false);
const dataList = ref<Task[]>([]);
const total = ref(0);
const page = ref(1);
const size = ref(20);
const statusFilter = ref<string>("");
const priorityFilter = ref<string>("");

const statusOptions = [
  { value: "todo", label: "待办" },
  { value: "in_progress", label: "进行中" },
  { value: "done", label: "完成" }
];

const priorityOptions = [
  { value: "low", label: "低", type: "info" },
  { value: "medium", label: "中", type: "" },
  { value: "high", label: "高", type: "danger" }
];

const dialogVisible = ref(false);
const form = ref<{
  title: string;
  description: string;
  status: "todo" | "in_progress" | "done";
  priority: "low" | "medium" | "high";
  assigneeId: number | null;
  dueDate: string;
  sortOrder: number | null;
}>({
  title: "",
  description: "",
  status: "todo",
  priority: "medium",
  assigneeId: null,
  dueDate: "",
  sortOrder: 0
});

const statusLabel = computed(() => (s: string) => statusOptions.find(o => o.value === s)?.label ?? s);
const priorityMeta = computed(() => (p: string) => priorityOptions.find(o => o.value === p));

async function fetchData() {
  loading.value = true;
  try {
    const params: Record<string, unknown> = { page: page.value, size: size.value };
    if (statusFilter.value) params.status = statusFilter.value;
    if (priorityFilter.value) params.priority = priorityFilter.value;
    const { data } = await listTasks(projectId.value, params);
    dataList.value = data.items;
    total.value = data.total;
  } finally {
    loading.value = false;
  }
}

async function fetchProject() {
  try {
    const { data } = await getProject(projectId.value);
    project.value = data;
  } catch {
    // project may be archived or not visible; we just skip the header
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
  form.value = {
    title: "",
    description: "",
    status: "todo",
    priority: "medium",
    assigneeId: null,
    dueDate: "",
    sortOrder: 0
  };
  dialogVisible.value = true;
}

async function submitCreate() {
  if (!form.value.title) {
    ElMessage.warning("标题不能为空");
    return;
  }
  const payload: Partial<Task> & { title: string } = {
    title: form.value.title,
    description: form.value.description || undefined,
    status: form.value.status,
    priority: form.value.priority,
    assigneeId: form.value.assigneeId ?? undefined,
    dueDate: form.value.dueDate || undefined,
    sortOrder: form.value.sortOrder ?? 0
  };
  await createTask(projectId.value, payload);
  ElMessage.success("任务创建成功");
  dialogVisible.value = false;
  await fetchData();
}

async function changeStatus(row: Task, newStatus: string) {
  if (row.status === newStatus) return;
  await updateTask(row.id, { status: newStatus as Task["status"] });
  ElMessage.success("状态已更新");
  await fetchData();
}

async function removeTask(row: Task) {
  await deleteTask(row.id);
  ElMessage.success("已删除");
  await fetchData();
}

onMounted(async () => {
  await fetchProject();
  await fetchData();
});
</script>

<template>
  <div class="pm-tasks">
    <el-page-header @back="router.back()">
      <template #content>
        <span>{{ project?.name ?? "项目" }} - 任务</span>
      </template>
      <template #extra>
        <el-button type="primary" @click="openCreate">新建任务</el-button>
      </template>
    </el-page-header>

    <div class="filters">
      <el-select
        v-model="statusFilter"
        placeholder="按状态筛选"
        clearable
        style="width: 160px"
        @change="fetchData"
      >
        <el-option v-for="o in statusOptions" :key="o.value" :label="o.label" :value="o.value" />
      </el-select>
      <el-select
        v-model="priorityFilter"
        placeholder="按优先级筛选"
        clearable
        style="width: 160px"
        @change="fetchData"
      >
        <el-option v-for="o in priorityOptions" :key="o.value" :label="o.label" :value="o.value" />
      </el-select>
    </div>

    <div v-loading="loading" style="margin-top: 16px">
      <el-empty v-if="dataList.length === 0" description="还没有任务" />
      <el-table v-else :data="dataList" stripe>
        <el-table-column prop="id" label="ID" width="60" />
        <el-table-column prop="title" label="标题" min-width="200" show-overflow-tooltip />
        <el-table-column label="状态" width="120">
          <template #default="{ row }">
            <el-select
              :model-value="row.status"
              size="small"
              style="width: 100px"
              @change="(v: string) => changeStatus(row, v)"
            >
              <el-option
                v-for="o in statusOptions"
                :key="o.value"
                :label="o.label"
                :value="o.value"
              />
            </el-select>
          </template>
        </el-table-column>
        <el-table-column label="优先级" width="100">
          <template #default="{ row }">
            <el-tag
              v-if="priorityMeta(row.priority)"
              :type="priorityMeta(row.priority)?.type"
              size="small"
            >
              {{ priorityMeta(row.priority)?.label }}
            </el-tag>
          </template>
        </el-table-column>
        <el-table-column prop="assigneeId" label="负责人" width="100">
          <template #default="{ row }">
            {{ row.assigneeId ?? "—" }}
          </template>
        </el-table-column>
        <el-table-column prop="dueDate" label="截止日期" width="120" />
        <el-table-column prop="createdAt" label="创建于" width="180" />
        <el-table-column label="操作" width="80" fixed="right">
          <template #default="{ row }">
            <el-button size="small" link type="danger" @click="removeTask(row)">删除</el-button>
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
    </div>

    <el-dialog v-model="dialogVisible" title="新建任务" width="520">
      <el-form label-width="80px">
        <el-form-item label="标题" required>
          <el-input v-model="form.title" placeholder="任务标题" />
        </el-form-item>
        <el-form-item label="描述">
          <el-input v-model="form.description" type="textarea" :rows="3" />
        </el-form-item>
        <el-form-item label="状态">
          <el-select v-model="form.status" style="width: 100%">
            <el-option v-for="o in statusOptions" :key="o.value" :label="o.label" :value="o.value" />
          </el-select>
        </el-form-item>
        <el-form-item label="优先级">
          <el-select v-model="form.priority" style="width: 100%">
            <el-option v-for="o in priorityOptions" :key="o.value" :label="o.label" :value="o.value" />
          </el-select>
        </el-form-item>
        <el-form-item label="负责人 ID">
          <el-input-number v-model="form.assigneeId" :min="0" style="width: 100%" />
        </el-form-item>
        <el-form-item label="截止日期">
          <el-input v-model="form.dueDate" placeholder="YYYY-MM-DD" />
        </el-form-item>
        <el-form-item label="排序">
          <el-input-number v-model="form.sortOrder" :min="0" style="width: 100%" />
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
.pm-tasks {
  padding: 16px;
}
.filters {
  margin-top: 16px;
  display: flex;
  gap: 12px;
}
.el-pagination {
  margin-top: 16px;
  justify-content: flex-end;
}
</style>

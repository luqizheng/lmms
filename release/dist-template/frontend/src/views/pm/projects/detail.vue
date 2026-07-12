<script setup lang="ts">
import { ref, onMounted, watch } from "vue";
import { useRoute, useRouter } from "vue-router";
import {
  listRequirements,
  createRequirement,
  deleteRequirement,
  type Requirement,
  type Paged
} from "@/api/pm";
import { ArrowLeft } from "@element-plus/icons-vue";
import { usePmStore } from "@/store/modules/pm";

defineOptions({ name: "PmProjectDetail" });

const route = useRoute();
const router = useRouter();
const pmStore = usePmStore();

const projectId = ref<number>(Number(route.params.id));
const loading = ref(false);
const list = ref<Requirement[]>([]);

const filterStatus = ref<string>("");
const filterType = ref<string>("");
const page = ref(1);
const size = ref(20);
const total = ref(0);

const typeLabel: Record<string, string> = {
  feature: "特性",
  improvement: "改进",
  bug: "缺陷",
  task: "任务"
};
const priorityTagType: Record<string, "danger" | "warning" | "primary" | "info"> = {
  p0: "danger",
  p1: "warning",
  p2: "primary",
  p3: "info"
};
const statusTagType: Record<string, "primary" | "warning" | "success" | "info"> = {
  draft: "info",
  open: "primary",
  in_progress: "warning",
  resolved: "success",
  closed: "info",
  rejected: "info"
};

const dialogVisible = ref(false);
const form = ref<{
  title: string;
  type: string;
  priority: string;
  status: string;
  description: string;
  estimateHours: number | null;
}>({
  title: "",
  type: "feature",
  priority: "p2",
  status: "draft",
  description: "",
  estimateHours: null
});

async function fetchData() {
  loading.value = true;
  try {
    const params: { page: number; size: number; status?: string; type?: string } = {
      page: page.value,
      size: size.value
    };
    if (filterStatus.value) params.status = filterStatus.value;
    if (filterType.value) params.type = filterType.value;
    const { data }: { data: Paged<Requirement> } = await listRequirements(projectId.value, params);
    list.value = data.items;
    total.value = data.total;
  } finally {
    loading.value = false;
  }
}

async function loadProject() {
  await pmStore.loadProject(projectId.value);
}

watch(() => route.params.id, async newId => {
  projectId.value = Number(newId);
  await Promise.all([loadProject(), fetchData()]);
});

function openCreate() {
  form.value = {
    title: "",
    type: "feature",
    priority: "p2",
    status: "draft",
    description: "",
    estimateHours: null
  };
  dialogVisible.value = true;
}

async function submitCreate() {
  if (!form.value.title) return;
  await createRequirement(projectId.value, form.value);
  dialogVisible.value = false;
  fetchData();
}

function goRequirement(row: Requirement) {
  router.push({ name: "PmRequirementDetail", params: { id: row.id } });
}

async function removeRequirement(row: Requirement) {
  await deleteRequirement(row.id);
  fetchData();
}

onMounted(async () => {
  await Promise.all([loadProject(), fetchData()]);
});
</script>

<template>
  <div class="pm-project-detail" v-loading="loading">
    <el-page-header :icon="ArrowLeft" @back="router.push({ name: 'PmProjectList' })">
      <template #content>
        <span class="title">
          {{ pmStore.currentProject?.name ?? `Project #${projectId}` }}
        </span>
        <el-tag size="small" style="margin-left: 8px">
          {{ pmStore.currentProject?.code }}
        </el-tag>
      </template>
    </el-page-header>

    <el-card v-if="pmStore.currentProject?.description" class="desc">
      {{ pmStore.currentProject.description }}
    </el-card>

    <el-card>
      <template #header>
        <div class="card-header">
          <span>需求 ({{ total }})</span>
          <div>
            <el-select v-model="filterStatus" placeholder="状态" clearable style="width: 120px; margin-right: 8px" @change="fetchData">
              <el-option label="draft" value="draft" />
              <el-option label="open" value="open" />
              <el-option label="in_progress" value="in_progress" />
              <el-option label="resolved" value="resolved" />
              <el-option label="closed" value="closed" />
            </el-select>
            <el-select v-model="filterType" placeholder="类型" clearable style="width: 120px; margin-right: 8px" @change="fetchData">
              <el-option label="feature" value="feature" />
              <el-option label="improvement" value="improvement" />
              <el-option label="bug" value="bug" />
              <el-option label="task" value="task" />
            </el-select>
            <el-button type="primary" @click="openCreate">新建需求</el-button>
          </div>
        </div>
      </template>

      <el-table :data="list" stripe>
        <el-table-column prop="id" label="ID" width="80" />
        <el-table-column prop="title" label="标题" min-width="220" />
        <el-table-column label="类型" width="100">
          <template #default="{ row }">
            {{ typeLabel[row?.type] ?? row?.type }}
          </template>
        </el-table-column>
        <el-table-column label="优先级" width="90">
          <template #default="{ row }">
            <el-tag :type="priorityTagType[row?.priority]" size="small">
              {{ row?.priority }}
            </el-tag>
          </template>
        </el-table-column>
        <el-table-column label="状态" width="120">
          <template #default="{ row }">
            <el-tag :type="statusTagType[row?.status]" size="small">
              {{ row?.status }}
            </el-tag>
          </template>
        </el-table-column>
        <el-table-column prop="assigneeId" label="指派" width="90" />
        <el-table-column prop="updatedAt" label="最近更新" width="180" />
        <el-table-column label="操作" width="160" fixed="right">
          <template #default="{ row }">
            <el-button size="small" link @click="goRequirement(row)">详情</el-button>
            <el-button size="small" link type="danger" @click="removeRequirement(row)">
              删除
            </el-button>
          </template>
        </el-table-column>
      </el-table>

      <el-pagination
        v-model:current-page="page"
        v-model:page-size="size"
        :total="total"
        layout="total, prev, pager, next"
        @current-change="fetchData"
      />
    </el-card>

    <el-dialog v-model="dialogVisible" title="新建需求" width="560">
      <el-form label-width="100px">
        <el-form-item label="标题" required>
          <el-input v-model="form.title" />
        </el-form-item>
        <el-form-item label="类型" required>
          <el-select v-model="form.type">
            <el-option label="feature (特性)" value="feature" />
            <el-option label="improvement (改进)" value="improvement" />
            <el-option label="bug (缺陷)" value="bug" />
            <el-option label="task (任务)" value="task" />
          </el-select>
        </el-form-item>
        <el-form-item label="优先级">
          <el-radio-group v-model="form.priority">
            <el-radio-button value="p0">P0</el-radio-button>
            <el-radio-button value="p1">P1</el-radio-button>
            <el-radio-button value="p2">P2</el-radio-button>
            <el-radio-button value="p3">P3</el-radio-button>
          </el-radio-group>
        </el-form-item>
        <el-form-item label="状态">
          <el-select v-model="form.status">
            <el-option label="draft" value="draft" />
            <el-option label="open" value="open" />
            <el-option label="in_progress" value="in_progress" />
          </el-select>
        </el-form-item>
        <el-form-item label="预估工时">
          <el-input-number v-model="form.estimateHours" :min="0" />
        </el-form-item>
        <el-form-item label="描述">
          <el-input v-model="form.description" type="textarea" :rows="4" />
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
.pm-project-detail {
  padding: 16px;
}
.title {
  font-size: 20px;
  font-weight: 600;
}
.desc {
  margin: 16px 0;
}
.card-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
}
.el-pagination {
  margin-top: 16px;
  justify-content: flex-end;
}
</style>

<script setup lang="ts">
import { ref, onMounted, watch } from "vue";
import { useRoute, useRouter } from "vue-router";
import {
  listStories,
  createStory,
  updateStory,
  listCriteria,
  createCriterion,
  updateCriterion,
  checkCriterion,
  type Requirement,
  type UserStory,
  type AcceptanceCriteria
} from "@/api/pm";
import { decomposeRequirement, refineRequirement } from "@/api/llm";
import { usePmStore } from "@/store/modules/pm";

defineOptions({ name: "PmRequirementDetail" });

const route = useRoute();
const router = useRouter();
const pmStore = usePmStore();

const reqId = ref<number>(Number(route.params.id));

const storyDialogVisible = ref(false);
const newStory = ref<{ asA: string; iWant: string; soThat: string; storyPoints: number | null }>({
  asA: "",
  iWant: "",
  soThat: "",
  storyPoints: null
});

const acDialogFor = ref<{ storyId: number; given: string; when: string; then: string } | null>(null);

const decomposeDialog = ref<{ open: boolean; content: string; disabled: boolean }>({
  open: false,
  content: "",
  disabled: true
});
const decomposeLoading = ref(false);

const storyStatusOptions = ["pending", "in_progress", "done", "rejected"];

async function refresh() {
  await pmStore.loadRequirement(reqId.value);
  if (pmStore.currentRequirement) {
    await pmStore.loadStories(reqId.value);
  }
}

watch(() => route.params.id, async n => {
  reqId.value = Number(n);
  await refresh();
});

async function loadCriteriaFor(storyId: number) {
  await pmStore.loadCriteria(storyId);
}

function openCreateStory() {
  newStory.value = { asA: "", iWant: "", soThat: "", storyPoints: null };
  storyDialogVisible.value = true;
}

async function submitCreateStory() {
  if (!newStory.value.iWant) return;
  await createStory(reqId.value, newStory.value);
  storyDialogVisible.value = false;
  await pmStore.loadStories(reqId.value);
}

async function changeStoryStatus(story: UserStory, status: string) {
  await updateStory(story.id, { status });
  await pmStore.loadStories(reqId.value);
}

function openCreateAc(story: UserStory) {
  acDialogFor.value = { storyId: story.id, given: "", when: "", then: "" };
}

async function submitCreateAc() {
  if (!acDialogFor.value) return;
  const { storyId, given, when, then } = acDialogFor.value;
  await createCriterion(storyId, { givenText: given, whenText: when, thenText: then });
  acDialogFor.value = null;
  await pmStore.loadCriteria(storyId);
}

async function toggleCriterion(c: AcceptanceCriteria) {
  await checkCriterion(c.id, !c.checked);
  // Optimistic refresh: refetch criteria for the owning story
  await pmStore.loadCriteria(c.userStoryId);
}

async function updateCriterionText(c: AcceptanceCriteria, field: "givenText" | "whenText" | "thenText", value: string) {
  await updateCriterion(c.id, { [field]: value });
}

async function aiDecompose() {
  decomposeLoading.value = true;
  try {
    const { data } = await decomposeRequirement(reqId.value);
    decomposeDialog.value = { open: true, content: data.content, disabled: data.disabled };
  } finally {
    decomposeLoading.value = false;
  }
}

async function aiRefine() {
  decomposeLoading.value = true;
  try {
    const { data } = await refineRequirement(reqId.value);
    decomposeDialog.value = { open: true, content: data.content, disabled: data.disabled };
  } finally {
    decomposeLoading.value = false;
  }
}

onMounted(refresh);
</script>

<template>
  <div class="pm-req-detail" v-if="pmStore.currentRequirement">
    <el-page-header :icon="ArrowLeft" @back="router.back()">
      <template #content>
        <span class="title">{{ pmStore.currentRequirement.title }}</span>
        <el-tag size="small" style="margin-left: 8px">{{ pmStore.currentRequirement.type }}</el-tag>
        <el-tag size="small" type="info" style="margin-left: 4px">
          {{ pmStore.currentRequirement.status }}
        </el-tag>
      </template>
      <template #extra>
        <el-button @click="aiRefine" :loading="decomposeLoading" disabled>
          AI 改写
        </el-button>
        <el-button type="primary" @click="aiDecompose" :loading="decomposeLoading" disabled>
          AI 拆解
        </el-button>
      </template>
    </el-page-header>

    <el-card v-if="pmStore.currentRequirement.description" class="desc">
      <pre>{{ pmStore.currentRequirement.description }}</pre>
    </el-card>

    <el-card>
      <template #header>
        <div class="card-header">
          <span>用户故事 ({{ pmStore.stories.length }})</span>
          <el-button type="primary" size="small" @click="openCreateStory">添加故事</el-button>
        </div>
      </template>

      <el-empty v-if="pmStore.stories.length === 0" description="还没有故事" />
      <div v-else>
        <div v-for="story in pmStore.stories" :key="story.id" class="story">
          <div class="story-head">
            <div>
              <span class="story-id">#{{ story.id }}</span>
              <strong>As a</strong> {{ story.asA || "—" }}
              <br />
              <strong>I want</strong> {{ story.iWant || "—" }}
              <br />
              <strong>So that</strong> {{ story.soThat || "—" }}
            </div>
            <div>
              <el-tag size="small" v-if="story.storyPoints !== null && story.storyPoints !== undefined">
                {{ story.storyPoints }} sp
              </el-tag>
              <el-select
                :model-value="story.status"
                size="small"
                style="width: 130px; margin-left: 8px"
                @update:modelValue="(v: string) => changeStoryStatus(story, v)"
              >
                <el-option v-for="s in storyStatusOptions" :key="s" :label="s" :value="s" />
              </el-select>
            </div>
          </div>

          <div class="ac-list">
            <div class="ac-header">
              <span>验收标准</span>
              <el-button link size="small" @click="loadCriteriaFor(story.id)">刷新</el-button>
              <el-button link size="small" @click="openCreateAc(story)">+ AC</el-button>
            </div>
            <ul v-if="pmStore.criteria.length > 0">
              <li
                v-for="c in pmStore.criteria.filter(a => a.userStoryId === story.id)"
                :key="c.id"
                class="ac-row"
              >
                <el-checkbox
                  :model-value="c.checked"
                  @update:modelValue="() => toggleCriterion(c)"
                />
                <span class="ac-given">Given {{ c.givenText || "—" }}</span>
                <span class="ac-when">When {{ c.whenText || "—" }}</span>
                <span class="ac-then">Then {{ c.thenText || "—" }}</span>
              </li>
            </ul>
            <div v-else class="ac-empty">未加载 AC</div>
          </div>
        </div>
      </div>
    </el-card>

    <el-dialog v-model="storyDialogVisible" title="添加用户故事" width="560">
      <el-form label-width="100px">
        <el-form-item label="As a">
          <el-input v-model="newStory.asA" placeholder="角色 / 干系人" />
        </el-form-item>
        <el-form-item label="I want" required>
          <el-input v-model="newStory.iWant" type="textarea" :rows="2" />
        </el-form-item>
        <el-form-item label="So that">
          <el-input v-model="newStory.soThat" type="textarea" :rows="2" />
        </el-form-item>
        <el-form-item label="故事点">
          <el-input-number v-model="newStory.storyPoints" :min="0" />
        </el-form-item>
      </el-form>
      <template #footer>
        <el-button @click="storyDialogVisible = false">取消</el-button>
        <el-button type="primary" @click="submitCreateStory">创建</el-button>
      </template>
    </el-dialog>

    <el-dialog
      v-model="acDialogFor"
      title="添加验收标准"
      width="520"
      @closed="acDialogFor = null"
    >
      <template v-if="acDialogFor">
        <el-form label-width="80px">
          <el-form-item label="Given"><el-input v-model="acDialogFor.given" /></el-form-item>
          <el-form-item label="When"><el-input v-model="acDialogFor.when" /></el-form-item>
          <el-form-item label="Then"><el-input v-model="acDialogFor.then" /></el-form-item>
        </el-form>
      </template>
      <template #footer>
        <el-button @click="acDialogFor = null">取消</el-button>
        <el-button type="primary" @click="submitCreateAc">创建</el-button>
      </template>
    </el-dialog>

    <el-dialog v-model="decomposeDialog.open" title="AI 助手输出" width="640">
      <el-alert
        v-if="decomposeDialog.disabled"
        type="info"
        :closable="false"
        title="MVP 暂未启用真实 LLM"
        description="当前仅 NoopLlmProvider 返回说明性文本。如需启用，请配置 pm.llm.provider=openai 等参数并在后端提供对应 Provider 实现。"
      />
      <pre class="ai-content">{{ decomposeDialog.content }}</pre>
      <template #footer>
        <el-button type="primary" @click="decomposeDialog.open = false">关闭</el-button>
      </template>
    </el-dialog>
  </div>
</template>

<style scoped>
.pm-req-detail {
  padding: 16px;
}
.title {
  font-size: 20px;
  font-weight: 600;
}
.desc pre {
  white-space: pre-wrap;
  margin: 0;
}
.card-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  font-weight: 500;
}
.story {
  border: 1px solid #ebeef5;
  border-radius: 6px;
  padding: 12px;
  margin-bottom: 12px;
}
.story-head {
  display: flex;
  justify-content: space-between;
  margin-bottom: 12px;
  align-items: flex-start;
}
.story-id {
  color: #909399;
  margin-right: 4px;
}
.ac-list {
  background: #f7f8fa;
  padding: 10px 12px;
  border-radius: 4px;
}
.ac-header {
  display: flex;
  gap: 8px;
  align-items: center;
  font-weight: 500;
  margin-bottom: 8px;
}
.ac-row {
  list-style: none;
  display: flex;
  gap: 12px;
  align-items: center;
  padding: 4px 0;
}
.ac-row > span {
  flex: 1 1 0;
}
.ac-empty {
  color: #909399;
  font-size: 13px;
}
.ai-content {
  white-space: pre-wrap;
  background: #f5f7fa;
  padding: 12px;
  border-radius: 4px;
  max-height: 60vh;
  overflow: auto;
}
.el-card {
  margin-bottom: 16px;
}
</style>

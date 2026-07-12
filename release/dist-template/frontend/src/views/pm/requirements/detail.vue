<script setup lang="ts">
import { ref, onMounted, watch, computed } from "vue";
import { useRoute, useRouter } from "vue-router";
import { ElMessage } from "element-plus";
import { ArrowLeft, Loading } from "@element-plus/icons-vue";
import {
  listStories,
  createStory,
  updateStory,
  listCriteria,
  createCriterion,
  updateCriterion,
  checkCriterion,
  updateRequirement,
  type Requirement,
  type UserStory,
  type AcceptanceCriteria
} from "@/api/pm";
import {
  decomposeRequirement,
  refineRequirement,
  parseDecomposedJson,
  type DecomposedStory,
  type CompletionResponse
} from "@/api/llm";
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
const acDialogVisible = computed({
  get: () => acDialogFor.value !== null,
  set: (v: boolean) => { if (!v) acDialogFor.value = null; }
});

type StoryStatus = UserStory["status"];
const storyStatusOptions: StoryStatus[] = ["pending", "in_progress", "done", "rejected"];

type AiMode = "decompose" | "refine" | null;

interface AiParsedStory extends DecomposedStory {
  selected: boolean;
}

const decomposeDialog = ref({
  open: false,
  mode: null as AiMode,
  loading: false,
  provider: "" as string,
  finishReason: "" as string,
  disabled: true,
  rawContent: "",
  errorMsg: "",
  // decompose-specific
  parsed: null as AiParsedStory[] | null,
  parseWarning: "",
  // refine-specific
  refinedText: ""
});

const importing = ref(false);
const applyingRefine = ref(false);

const allSelected = computed(() => {
  const stories = decomposeDialog.value.parsed;
  return !!stories && stories.length > 0 && stories.every(s => s.selected);
});

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

async function changeStoryStatus(story: UserStory, status: StoryStatus) {
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
  await pmStore.loadCriteria(c.userStoryId);
}

async function updateCriterionText(c: AcceptanceCriteria, field: "givenText" | "whenText" | "thenText", value: string) {
  await updateCriterion(c.id, { [field]: value });
}

function resetAiDialog() {
  decomposeDialog.value = {
    open: false,
    mode: null,
    loading: false,
    provider: "",
    finishReason: "",
    disabled: true,
    rawContent: "",
    errorMsg: "",
    parsed: null,
    parseWarning: "",
    refinedText: ""
  };
}

function applyAiResponse(mode: AiMode, resp: CompletionResponse) {
  decomposeDialog.value.mode = mode;
  decomposeDialog.value.provider = resp.provider;
  decomposeDialog.value.finishReason = resp.finishReason;
  decomposeDialog.value.disabled = resp.disabled;
  decomposeDialog.value.rawContent = resp.content ?? "";

  if (resp.disabled || resp.finishReason === "error") {
    decomposeDialog.value.errorMsg = resp.content;
    return;
  }

  if (mode === "refine") {
    decomposeDialog.value.refinedText = resp.content ?? "";
    return;
  }

  // decompose
  const parsed = parseDecomposedJson(resp.content ?? "");
  if (parsed && parsed.stories.length > 0) {
    decomposeDialog.value.parsed = parsed.stories.map(s => ({
      ...s,
      selected: true
    }));
    decomposeDialog.value.parseWarning = "";
  } else {
    decomposeDialog.value.parsed = null;
    decomposeDialog.value.parseWarning = "AI 返回内容不是合法的 stories JSON，已按原文展示。";
  }
}

async function runAi(mode: AiMode) {
  resetAiDialog();
  decomposeDialog.value.open = true;
  decomposeDialog.value.loading = true;
  decomposeDialog.value.mode = mode;
  try {
    const { data } = mode === "decompose"
      ? await decomposeRequirement(reqId.value)
      : await refineRequirement(reqId.value);
    applyAiResponse(mode, data);
  } catch (err: any) {
    decomposeDialog.value.disabled = true;
    decomposeDialog.value.finishReason = "error";
    decomposeDialog.value.errorMsg =
      err?.response?.data?.message || err?.message || String(err);
  } finally {
    decomposeDialog.value.loading = false;
  }
}

async function aiDecompose() {
  await runAi("decompose");
}

async function aiRefine() {
  await runAi("refine");
}

function toggleAllStories(checked: boolean) {
  decomposeDialog.value.parsed?.forEach(s => (s.selected = checked));
}

async function importSelectedStories() {
  const stories = decomposeDialog.value.parsed;
  if (!stories || stories.length === 0) return;
  const selected = stories.filter(s => s.selected && (s.i_want || s.title));
  if (selected.length === 0) {
    ElMessage.warning("请至少勾选一个故事");
    return;
  }
  importing.value = true;
  let storyCount = 0;
  let acCount = 0;
  try {
    for (const s of selected) {
      const title = s.title || s.i_want || "Untitled";
      const { data: createdStory } = await createStory(reqId.value, {
        asA: extractAsA(s.title, s.i_want),
        iWant: s.i_want || title,
        soThat: s.so_that || ""
      });
      storyCount++;
      const criteria = s.criteria || [];
      for (const c of criteria) {
        if (!c.given && !c.when && !c.then) continue;
        await createCriterion(createdStory.id, {
          givenText: c.given || "",
          whenText: c.when || "",
          thenText: c.then || ""
        });
        acCount++;
      }
    }
    ElMessage.success(`已导入 ${storyCount} 个故事，${acCount} 条验收标准`);
    decomposeDialog.value.open = false;
    await refresh();
  } catch (err: any) {
    ElMessage.error(`导入失败：${err?.response?.data?.message || err?.message || err}`);
  } finally {
    importing.value = false;
  }
}

async function applyRefinedDescription() {
  if (!decomposeDialog.value.refinedText) return;
  applyingRefine.value = true;
  try {
    await updateRequirement(reqId.value, { description: decomposeDialog.value.refinedText });
    ElMessage.success("描述已更新");
    decomposeDialog.value.open = false;
    await refresh();
  } catch (err: any) {
    ElMessage.error(`更新失败：${err?.response?.data?.message || err?.message || err}`);
  } finally {
    applyingRefine.value = false;
  }
}

/**
 * Best-effort extract a role phrase from a combined title string like
 * "As a 开发者, I want ..." — returns undefined on failure so caller uses
 * the empty default.
 */
function extractAsA(title: string | undefined, iWant: string | undefined): string | undefined {
  const source = title || "";
  const m = source.match(/As\s+a\s+([^,，.。]+?)(?:,|，|I\s+want|$)/i);
  if (m) return m[1].trim();
  // Fall back: if iWant starts with "As a ..." it might be combined; skip.
  void iWant;
  return undefined;
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
        <el-button @click="aiRefine" :loading="decomposeDialog.loading">
          AI 改写
        </el-button>
        <el-button type="primary" @click="aiDecompose" :loading="decomposeDialog.loading">
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
                @update:modelValue="(v: any) => changeStoryStatus(story, v as StoryStatus)"
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
      v-model="acDialogVisible"
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

    <el-dialog
      v-model="decomposeDialog.open"
      :title="decomposeDialog.mode === 'refine' ? 'AI 改写建议' : 'AI 拆解结果'"
      width="720"
      @closed="resetAiDialog"
    >
      <div v-if="decomposeDialog.loading" class="ai-loading">
        <el-icon class="is-loading"><Loading /></el-icon>
        <span>{{ decomposeDialog.mode === 'refine' ? '正在调用 AI 改写…' : '正在调用 AI 拆解…' }}</span>
      </div>

      <template v-else>
        <!-- Disabled / Error states -->
        <el-alert
          v-if="decomposeDialog.disabled || decomposeDialog.finishReason === 'error'"
          :type="decomposeDialog.finishReason === 'error' ? 'error' : 'info'"
          :closable="false"
          show-icon
          :title="decomposeDialog.finishReason === 'error' ? 'AI 调用失败' : 'LLM 未启用'"
          style="margin-bottom: 12px"
        >
          <template #default>
            {{ decomposeDialog.errorMsg || '请在 application.yml 或环境变量 DEEPSEEK_API_KEY 配置 pm.llm.deepseek.api-key，再把 pm.llm.provider.default 设为 deepseek。' }}
          </template>
        </el-alert>

        <!-- Refine success: show rewritten text + apply button -->
        <template v-else-if="decomposeDialog.mode === 'refine'">
          <div class="ai-refine-preview">
            <div class="ai-section-label">AI 改写后的描述：</div>
            <pre class="ai-content">{{ decomposeDialog.refinedText }}</pre>
          </div>
        </template>

        <!-- Decompose success: parsed JSON or raw fallback -->
        <template v-else-if="decomposeDialog.mode === 'decompose'">
          <el-alert
            v-if="decomposeDialog.parseWarning"
            type="warning"
            :closable="false"
            show-icon
            :title="decomposeDialog.parseWarning"
            style="margin-bottom: 12px"
          />

          <div v-if="decomposeDialog.parsed && decomposeDialog.parsed.length > 0">
            <div class="ai-toolbar">
              <el-checkbox
                :model-value="allSelected"
                @change="toggleAllStories"
              >
                全选
              </el-checkbox>
              <span class="ai-stats">
                共 {{ decomposeDialog.parsed.length }} 个故事，
                已选 {{ decomposeDialog.parsed.filter(s => s.selected).length }} 个
              </span>
            </div>

            <div class="ai-story-list">
              <div
                v-for="(s, idx) in decomposeDialog.parsed"
                :key="idx"
                class="ai-story-card"
              >
                <div class="ai-story-head">
                  <el-checkbox v-model="s.selected" />
                  <span class="ai-story-title">{{ s.title || s.i_want || `故事 ${idx + 1}` }}</span>
                </div>
                <div v-if="s.i_want" class="ai-story-line"><strong>I want</strong> {{ s.i_want }}</div>
                <div v-if="s.so_that" class="ai-story-line"><strong>So that</strong> {{ s.so_that }}</div>
                <div v-if="s.criteria && s.criteria.length > 0" class="ai-criteria">
                  <div class="ai-criteria-title">验收标准 ({{ s.criteria.length }})</div>
                  <ul>
                    <li v-for="(c, ci) in s.criteria" :key="ci">
                      <span v-if="c.given">Given {{ c.given }}<span v-if="c.when || c.then">；</span></span>
                      <span v-if="c.when">When {{ c.when }}<span v-if="c.then">；</span></span>
                      <span v-if="c.then">Then {{ c.then }}</span>
                    </li>
                  </ul>
                </div>
              </div>
            </div>
          </div>

          <!-- Raw fallback when JSON parse failed -->
          <pre v-else class="ai-content">{{ decomposeDialog.rawContent }}</pre>
        </template>
      </template>

      <template #footer>
        <el-button @click="decomposeDialog.open = false">关闭</el-button>
        <el-button
          v-if="!decomposeDialog.loading && !decomposeDialog.disabled && decomposeDialog.finishReason !== 'error' && decomposeDialog.mode === 'refine'"
          type="primary"
          :loading="applyingRefine"
          @click="applyRefinedDescription"
        >
          应用改写
        </el-button>
        <el-button
          v-if="!decomposeDialog.loading && !decomposeDialog.disabled && decomposeDialog.finishReason !== 'error' && decomposeDialog.mode === 'decompose' && decomposeDialog.parsed && decomposeDialog.parsed.length > 0"
          type="primary"
          :loading="importing"
          @click="importSelectedStories"
        >
          批量导入已勾选
        </el-button>
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
.ai-loading {
  display: flex;
  align-items: center;
  gap: 8px;
  color: #606266;
  padding: 24px;
  justify-content: center;
}
.ai-content {
  white-space: pre-wrap;
  background: #f5f7fa;
  padding: 12px;
  border-radius: 4px;
  max-height: 60vh;
  overflow: auto;
  margin: 0;
  font-family: Menlo, Consolas, monospace;
  font-size: 13px;
}
.ai-refine-preview {
  display: flex;
  flex-direction: column;
  gap: 8px;
}
.ai-section-label {
  font-weight: 500;
  color: #303133;
}
.ai-toolbar {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 12px;
  padding: 8px 12px;
  background: #f5f7fa;
  border-radius: 4px;
}
.ai-stats {
  font-size: 13px;
  color: #909399;
}
.ai-story-list {
  max-height: 60vh;
  overflow-y: auto;
  display: flex;
  flex-direction: column;
  gap: 10px;
}
.ai-story-card {
  border: 1px solid #ebeef5;
  border-radius: 6px;
  padding: 10px 12px;
  background: #fff;
}
.ai-story-head {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-bottom: 6px;
}
.ai-story-title {
  font-weight: 600;
  color: #303133;
}
.ai-story-line {
  font-size: 13px;
  color: #606266;
  margin-bottom: 4px;
}
.ai-story-line strong {
  color: #303133;
  margin-right: 4px;
}
.ai-criteria {
  margin-top: 6px;
  padding-top: 6px;
  border-top: 1px dashed #ebeef5;
  font-size: 13px;
}
.ai-criteria-title {
  color: #909399;
  margin-bottom: 4px;
}
.ai-criteria ul {
  margin: 0;
  padding-left: 18px;
  color: #606266;
}
.ai-criteria li {
  margin-bottom: 2px;
}
.el-card {
  margin-bottom: 16px;
}
</style>

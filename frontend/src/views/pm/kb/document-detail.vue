<script setup lang="ts">
import { ref, onMounted, computed } from "vue";
import { useRoute, useRouter } from "vue-router";
import { ElMessage, ElMessageBox } from "element-plus";
import { MdEditor, MdPreview } from "md-editor-v3";
import "md-editor-v3/lib/style.css";
import {
  getDocument,
  getLatestVersion,
  updateDocumentContent,
  listVersions,
  getVersion,
  rollbackVersion,
  listComments,
  createComment,
  type KbDocument,
  type KbDocumentVersion,
  type Comment
} from "@/api/kb";
import { uploadAttachment } from "@/api/pm";

defineOptions({ name: "PmKbDocumentDetail" });

const route = useRoute();
const router = useRouter();
const docId = ref<number>(Number(route.params.id));

const doc = ref<KbDocument | null>(null);
const loading = ref(false);
const saving = ref(false);
const editing = ref(false);
const content = ref("");
const title = ref("");
const editSummary = ref("");
const versions = ref<KbDocumentVersion[]>([]);
const viewingVersion = ref<KbDocumentVersion | null>(null);
const versionsPanelOpen = ref(false);
const uploading = ref(false);
const attachments = ref<{ id: number; name: string }[]>([]);
const fileInput = ref<HTMLInputElement | null>(null);

const comments = ref<Comment[]>([]);
const commentLoading = ref(false);
const newComment = ref("");
const commentSaving = ref(false);

const displayContent = computed(() => {
  if (viewingVersion.value) return viewingVersion.value.contentMd ?? "";
  return content.value;
});

const displayTitle = computed(() => {
  if (viewingVersion.value) return viewingVersion.value.title;
  return title.value;
});

async function loadDocument() {
  loading.value = true;
  try {
    const { data: d } = await getDocument(docId.value);
    doc.value = d;
    title.value = d.title;
    const { data: latest } = await getLatestVersion(docId.value);
    content.value = latest.contentMd ?? "";
    await loadVersions();
  } catch (err: any) {
    ElMessage.error("加载文档失败: " + (err?.message ?? err));
  } finally {
    loading.value = false;
  }
}

async function loadVersions() {
  try {
    const { data } = await listVersions(docId.value);
    versions.value = data;
  } catch {
    versions.value = [];
  }
}

function startEdit() {
  if (viewingVersion.value) {
    content.value = viewingVersion.value.contentMd ?? "";
    title.value = viewingVersion.value.title;
    viewingVersion.value = null;
  }
  editSummary.value = "";
  editing.value = true;
}

function cancelEdit() {
  editing.value = false;
}

async function save() {
  if (!content.value.trim()) {
    ElMessage.warning("内容不能为空");
    return;
  }
  saving.value = true;
  try {
    await updateDocumentContent(docId.value, {
      markdown: content.value,
      title: title.value,
      editSummary: editSummary.value || undefined
    });
    ElMessage.success("保存成功，新版本已创建");
    editing.value = false;
    await loadDocument();
  } catch (err: any) {
    ElMessage.error("保存失败: " + (err?.message ?? err));
  } finally {
    saving.value = false;
  }
}

async function viewVersion(v: KbDocumentVersion) {
  try {
    const { data } = await getVersion(docId.value, v.versionNo);
    viewingVersion.value = data;
    editing.value = false;
  } catch (err: any) {
    ElMessage.error("加载版本失败: " + (err?.message ?? err));
  }
}

function backToLatest() {
  viewingVersion.value = null;
}

async function doRollback(v: KbDocumentVersion) {
  try {
    await ElMessageBox.confirm(
      `确定回滚到版本 ${v.versionNo}？这将创建一个新版本，内容来自版本 ${v.versionNo}。`,
      "版本回滚",
      { type: "warning" }
    );
  } catch {
    return;
  }
  try {
    await rollbackVersion(docId.value, v.versionNo);
    ElMessage.success("回滚成功");
    viewingVersion.value = null;
    await loadDocument();
  } catch (err: any) {
    ElMessage.error("回滚失败: " + (err?.message ?? err));
  }
}

onMounted(() => {
  loadDocument();
  loadComments();
});

async function loadComments() {
  if (!docId.value) return;
  commentLoading.value = true;
  try {
    const { data } = await listComments(docId.value);
    comments.value = data;
  } catch { comments.value = []; }
  finally { commentLoading.value = false; }
}

async function addComment() {
  if (!newComment.value.trim()) return;
  commentSaving.value = true;
  try {
    await createComment(docId.value, { content: newComment.value });
    newComment.value = "";
    await loadComments();
  } catch (err: any) {
    ElMessage.error("评论失败: " + (err?.message ?? err));
  } finally {
    commentSaving.value = false;
  }
}

async function handleUploadFile(event: Event) {
  const input = event.target as HTMLInputElement;
  const file = input?.files?.[0];
  if (!file) return;
  uploading.value = true;
  try {
    const tag = `kb:doc:${docId.value}`;
    const { data } = await uploadAttachment(file, tag);
    attachments.value.push(data);
    ElMessage.success(`附件 ${data.name} 上传成功`);
  } catch (err: any) {
    ElMessage.error("上传失败: " + (err?.message ?? err));
  } finally {
    uploading.value = false;
    input.value = "";
  }
}

function triggerUpload() {
  fileInput.value?.click();
}
</script>

<template>
  <div class="pm-kb-doc-detail" v-loading="loading">
    <el-page-header @back="router.back()">
      <template #content>
        <span class="doc-title">{{ displayTitle || doc?.title || "加载中…" }}</span>
        <el-tag v-if="viewingVersion" size="small" type="warning" style="margin-left: 8px">
          v{{ viewingVersion.versionNo }}
        </el-tag>
        <el-tag v-else-if="doc" size="small" style="margin-left: 8px">最新</el-tag>
      </template>
      <template #extra>
        <el-button v-if="viewingVersion" @click="backToLatest">返回最新版本</el-button>
        <el-button v-if="!editing && !viewingVersion" type="primary" @click="startEdit">编辑</el-button>
        <template v-if="editing">
          <el-button @click="cancelEdit">取消</el-button>
          <el-button :loading="uploading" @click="triggerUpload">上传附件</el-button>
          <el-button type="primary" :loading="saving" @click="save">保存</el-button>
          <input ref="fileInput" type="file" style="display:none" @change="handleUploadFile" />
        </template>
      </template>
    </el-page-header>

    <!-- Edit summary when editing -->
    <div v-if="editing" class="edit-bar">
      <el-input
        v-model="editSummary"
        placeholder="编辑摘要（可选，如：修正格式、补充内容）"
        size="small"
        clearable
      />
    </div>

    <!-- Editor (edit mode) -->
    <div v-if="editing" class="editor-wrap">
      <el-input
        v-model="title"
        placeholder="文档标题"
        size="large"
        style="margin-bottom: 12px"
      />
      <MdEditor
        v-model="content"
        :toolbars-exclude="['github', 'save', 'pageFullscreen', 'htmlPreview', 'catalog']"
        :preview="true"
        style="height: calc(100vh - 280px)"
      />
    </div>

    <!-- Preview (view mode) -->
    <div v-else class="preview-wrap">
      <MdPreview :modelValue="displayContent" />
    </div>

    <!-- Version history panel -->
    <el-drawer v-model="versionsPanelOpen" title="版本历史" size="400px">
      <div v-if="versions.length === 0" class="empty-hint">暂无版本记录</div>
      <el-timeline v-else>
        <el-timeline-item
          v-for="v in versions"
          :key="v.id"
          :timestamp="v.createdAt"
          placement="top"
          :type="viewingVersion?.versionNo === v.versionNo ? 'primary' : ''"
        >
          <div class="version-card">
            <div class="version-head">
              <span class="version-no">v{{ v.versionNo }}</span>
              <span class="version-title">{{ v.title }}</span>
            </div>
            <div v-if="v.editSummary" class="version-summary">{{ v.editSummary }}</div>
            <div class="version-actions">
              <el-button size="small" link @click="viewVersion(v)">查看</el-button>
              <el-button
                v-if="!viewingVersion || viewingVersion.versionNo !== v.versionNo"
                size="small"
                link
                type="warning"
                @click="doRollback(v)"
              >
                回滚到此版本
              </el-button>
            </div>
          </div>
        </el-timeline-item>
      </el-timeline>
    </el-drawer>

    <!-- Floating version button -->
    <el-button
      class="version-fab"
      type="info"
      circle
      @click="versionsPanelOpen = true"
    >
      <el-icon><Clock /></el-icon>
    </el-button>

    <!-- Comment section -->
    <div class="comment-section">
      <div class="comment-header">评论 ({{ comments.length }})</div>

      <div v-if="commentLoading" class="comment-loading">加载中…</div>
      <div v-else>
        <div v-if="comments.length === 0" class="comment-empty">暂无评论</div>
        <div v-for="c in comments" :key="c.id" class="comment-item">
          <div class="comment-meta">用户 #{{ c.authorId }} · {{ c.createdAt }}</div>
          <div class="comment-content">{{ c.content }}</div>
        </div>
      </div>

      <div class="comment-input-row">
        <el-input
          v-model="newComment"
          type="textarea"
          :rows="2"
          placeholder="写下你的评论…"
          :disabled="commentSaving"
        />
        <el-button
          type="primary"
          :loading="commentSaving"
          :disabled="!newComment.trim()"
          @click="addComment"
          style="margin-top: 8px"
        >
          发表评论
        </el-button>
      </div>
    </div>
  </div>
</template>

<script lang="ts">
import { Clock } from "@element-plus/icons-vue";
export default { components: { Clock } };
</script>

<style scoped>
.pm-kb-doc-detail {
  padding: 16px;
  min-height: calc(100vh - 120px);
}
.doc-title {
  font-size: 20px;
  font-weight: 600;
}
.edit-bar {
  margin: 12px 0;
}
.editor-wrap {
  margin-top: 12px;
}
.preview-wrap {
  margin-top: 16px;
  background: #fff;
  border-radius: 6px;
  padding: 24px;
  min-height: 400px;
}
.version-card {
  padding: 4px 0;
}
.version-head {
  display: flex;
  gap: 8px;
  align-items: baseline;
}
.version-no {
  font-weight: 600;
  color: #409eff;
}
.version-title {
  color: #303133;
}
.version-summary {
  font-size: 13px;
  color: #909399;
  margin: 4px 0;
}
.version-actions {
  margin-top: 4px;
}
.empty-hint {
  color: #909399;
  text-align: center;
  padding: 24px;
}
.version-fab {
  position: fixed;
  right: 32px;
  bottom: 80px;
  z-index: 100;
}
.comment-section {
  margin-top: 32px;
  border-top: 1px solid #ebeef5;
  padding-top: 16px;
}
.comment-header {
  font-size: 16px;
  font-weight: 600;
  margin-bottom: 12px;
}
.comment-loading, .comment-empty {
  color: #909399;
  padding: 12px 0;
}
.comment-item {
  background: #f7f8fa;
  border-radius: 6px;
  padding: 10px 12px;
  margin-bottom: 8px;
}
.comment-meta {
  font-size: 12px;
  color: #909399;
  margin-bottom: 4px;
}
.comment-content {
  font-size: 14px;
  white-space: pre-wrap;
}
.comment-input-row {
  margin-top: 12px;
}
</style>

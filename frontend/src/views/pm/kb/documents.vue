<script setup lang="ts">
import { ref, onMounted } from "vue";
import { useRoute, useRouter } from "vue-router";
import { ElMessage } from "element-plus";
import {
  listDocuments,
  createDocument,
  getSpace,
  type KbDocument,
  type KbSpace
} from "@/api/kb";

defineOptions({ name: "PmKbDocuments" });

const route = useRoute();
const router = useRouter();
const spaceId = ref<number>(Number(route.params.spaceId));

const space = ref<KbSpace | null>(null);
const documents = ref<KbDocument[]>([]);
const loading = ref(false);

const dialogVisible = ref(false);
const form = ref({ title: "", markdown: "", docType: "markdown" });

async function fetchData() {
  loading.value = true;
  try {
    const { data: s } = await getSpace(spaceId.value);
    space.value = s;
    const { data: docs } = await listDocuments(spaceId.value);
    documents.value = docs;
  } catch (err: any) {
    ElMessage.error("加载失败: " + (err?.message ?? err));
  } finally {
    loading.value = false;
  }
}

function openCreate() {
  form.value = { title: "", markdown: "", docType: "markdown" };
  dialogVisible.value = true;
}

async function submitCreate() {
  if (!form.value.title) return;
  try {
    await createDocument(spaceId.value, form.value);
    ElMessage.success("文档创建成功");
    dialogVisible.value = false;
    await fetchData();
  } catch (err: any) {
    ElMessage.error("创建失败: " + (err?.message ?? err));
  }
}

function openDocument(doc: KbDocument) {
  router.push(`/pm/kb/documents/${doc.id}`);
}

onMounted(fetchData);
</script>

<template>
  <div class="pm-kb-docs">
    <el-page-header @back="router.back()">
      <template #content>
        <span>{{ space?.name ?? "知识库" }} - 文档列表</span>
      </template>
      <template #extra>
        <el-button type="primary" @click="openCreate">新建文档</el-button>
      </template>
    </el-page-header>

    <div v-loading="loading" style="margin-top: 16px">
      <el-empty v-if="documents.length === 0" description="还没有文档" />
      <div v-else class="doc-grid">
        <div
          v-for="doc in documents"
          :key="doc.id"
          class="doc-card"
          @click="openDocument(doc)"
        >
          <div class="doc-card-title">{{ doc.title }}</div>
          <div class="doc-card-meta">
            <el-tag size="small">{{ doc.docType }}</el-tag>
            <span class="doc-card-date">{{ doc.updatedAt }}</span>
          </div>
        </div>
      </div>
    </div>

    <el-dialog v-model="dialogVisible" title="新建文档" width="520">
      <el-form label-width="80px">
        <el-form-item label="标题" required>
          <el-input v-model="form.title" placeholder="文档标题" />
        </el-form-item>
        <el-form-item label="类型">
          <el-select v-model="form.docType" style="width: 100%">
            <el-option label="Markdown" value="markdown" />
            <el-option label="API 规范" value="api_spec" />
            <el-option label="设计文档" value="design_doc" />
            <el-option label="会议纪要" value="meeting" />
          </el-select>
        </el-form-item>
        <el-form-item label="初始内容">
          <el-input
            v-model="form.markdown"
            type="textarea"
            :rows="5"
            placeholder="可选，留空则创建空白文档"
          />
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
.pm-kb-docs {
  padding: 16px;
}
.doc-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(240px, 1fr));
  gap: 16px;
}
.doc-card {
  border: 1px solid #ebeef5;
  border-radius: 6px;
  padding: 16px;
  cursor: pointer;
  transition: box-shadow 0.2s;
}
.doc-card:hover {
  box-shadow: 0 2px 12px rgba(0, 0, 0, 0.1);
}
.doc-card-title {
  font-size: 16px;
  font-weight: 600;
  margin-bottom: 8px;
  color: #303133;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.doc-card-meta {
  display: flex;
  align-items: center;
  gap: 8px;
}
.doc-card-date {
  font-size: 12px;
  color: #909399;
}
</style>

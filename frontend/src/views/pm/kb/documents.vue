<script setup lang="ts">
import { ref, onMounted } from "vue";
import { useRoute, useRouter } from "vue-router";
import { ElMessage } from "element-plus";
import type Node from "element-plus/es/components/tree/src/model/node";
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
const loading = ref(false);
const treeRef = ref();

const dialogVisible = ref(false);
const form = ref({ title: "", markdown: "", docType: "markdown", parentId: null as number | null });

interface TreeNode {
  id: number;
  label: string;
  isLeaf: boolean;
  docType?: string;
  raw: KbDocument;
  children?: TreeNode[];
}

async function loadSpace() {
  try {
    const { data: s } = await getSpace(spaceId.value);
    space.value = s;
  } catch (err: any) {
    ElMessage.error("加载空间失败: " + (err?.message ?? err));
  }
}

async function loadChildren(node: Node, resolve: (data: TreeNode[]) => void) {
  const parentId = node.level === 0 ? undefined : node.data?.id;
  try {
    const { data: docs } = await listDocuments(spaceId.value, parentId);
    resolve(
      docs.map((d: KbDocument) => ({
        id: d.id,
        label: d.title,
        isLeaf: false,
        docType: d.docType,
        raw: d
      }))
    );
  } catch {
    resolve([]);
  }
}

function openCreate(parentId?: number) {
  form.value = { title: "", markdown: "", docType: "markdown", parentId: parentId ?? null };
  dialogVisible.value = true;
}

async function submitCreate() {
  if (!form.value.title) return;
  try {
    await createDocument(spaceId.value, form.value);
    ElMessage.success("文档创建成功");
    dialogVisible.value = false;
    treeRef.value?.refresh();
  } catch (err: any) {
    ElMessage.error("创建失败: " + (err?.message ?? err));
  }
}

function handleNodeClick(data: TreeNode) {
  router.push(`/pm/kb/documents/${data.id}`);
}

const docTypeTag: Record<string, "" | "primary" | "success" | "warning" | "info" | "danger"> = {
  markdown: "",
  api_spec: "success",
  design_doc: "warning",
  meeting: "info"
};

onMounted(loadSpace);
</script>

<template>
  <div class="pm-kb-docs">
    <el-page-header @back="router.back()">
      <template #content>
        <span>{{ space?.name ?? "知识库" }} - 文档树</span>
      </template>
      <template #extra>
        <el-button type="primary" @click="openCreate()">新建文档</el-button>
      </template>
    </el-page-header>

    <div v-loading="loading" class="tree-wrap">
      <el-tree
        ref="treeRef"
        :props="{ label: 'label', isLeaf: 'isLeaf' }"
        :load="loadChildren"
        lazy
        node-key="id"
        highlight-current
        :expand-on-click-node="false"
        @node-click="handleNodeClick"
      >
        <template #default="{ data }">
          <span class="tree-node">
            <span class="tree-node-label">{{ data.label }}</span>
            <el-tag v-if="data.docType && data.docType !== 'markdown'" size="small" :type="(docTypeTag as any)[data.docType]">
              {{ data.docType }}
            </el-tag>
          </span>
        </template>
      </el-tree>
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
.tree-wrap {
  margin-top: 16px;
  min-height: 300px;
}
.tree-node {
  display: flex;
  align-items: center;
  gap: 8px;
}
.tree-node-label {
  font-size: 14px;
}
</style>
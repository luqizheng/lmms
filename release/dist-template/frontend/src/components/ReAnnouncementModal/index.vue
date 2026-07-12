<script setup lang="ts">
import { ref, watch } from "vue";
import { useRouter } from "vue-router";
import { useNotificationStore } from "@/store/modules/notification";
import { getAnnouncement } from "@/api/notification";

interface AnnouncementDetail {
  id: number;
  title: string;
  body: string;
  publish_at: string;
  expire_at: string | null;
  pinned: boolean;
  popover: boolean;
}

const props = defineProps<{
  show: boolean;
  announcementId?: number;
}>();

const emit = defineEmits<{
  (e: "close"): void;
}>();

const router = useRouter();
const store = useNotificationStore();
const loading = ref(false);
const announcement = ref<AnnouncementDetail | null>(null);

watch(
  () => [props.show, props.announcementId],
  async ([show, id]) => {
    if (show && typeof id === "number") {
      loading.value = true;
      try {
        announcement.value = await getAnnouncement(id);
      } catch {
        emit("close");
      } finally {
        loading.value = false;
      }
    }
  },
  { immediate: true }
);

async function handleClose() {
  if (announcement.value) {
    try {
      await store.readOne(announcement.value.id);
    } catch {}
  }
  emit("close");
}

function handleViewDetail() {
  if (announcement.value) {
    router.push(`/notification/detail/${announcement.value.id}`);
    handleClose();
  }
}
</script>

<template>
  <Teleport to="body">
    <Transition name="fade">
      <div
        v-if="show"
        class="announcement-modal-overlay"
        @click.self="handleClose"
      >
        <div class="announcement-modal">
          <div class="modal-header">
            <div class="header-left">
              <el-tag v-if="announcement?.pinned" type="warning">置顶</el-tag>
              <h3>{{ announcement?.title ?? "加载中..." }}</h3>
            </div>
            <button class="close-btn" @click="handleClose">
              <span class="close-icon">×</span>
            </button>
          </div>
          <div v-if="!loading && announcement" class="modal-body">
            <div class="publish-info">
              <span>发布时间：{{ announcement.publish_at }}</span>
              <span v-if="announcement.expire_at">
                · 过期时间：{{ announcement.expire_at }}
              </span>
            </div>
            <div class="content">{{ announcement.body }}</div>
          </div>
          <div v-else-if="loading" class="modal-body">
            <div class="loading">加载中...</div>
          </div>
          <div class="modal-footer">
            <el-button @click="handleClose">关闭</el-button>
            <el-button type="primary" @click="handleViewDetail"
              >查看详情</el-button
            >
          </div>
        </div>
      </div>
    </Transition>
  </Teleport>
</template>

<style lang="scss" scoped>
.announcement-modal-overlay {
  position: fixed;
  inset: 0;
  z-index: 9999;
  display: flex;
  align-items: center;
  justify-content: center;
  background: rgb(0 0 0 / 50%);
}

.announcement-modal {
  width: 500px;
  max-width: 90%;
  overflow: hidden;
  background: #fff;
  border-radius: 8px;
  box-shadow: 0 4px 20px rgb(0 0 0 / 15%);
}

.modal-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 16px 20px;
  border-bottom: 1px solid #eee;

  .header-left {
    display: flex;
    gap: 10px;
    align-items: center;
  }

  h3 {
    margin: 0;
    font-size: 16px;
    font-weight: 600;
  }

  .close-btn {
    padding: 4px;
    color: #999;
    cursor: pointer;
    background: none;
    border: none;

    &:hover {
      color: #666;
    }
  }
}

.modal-body {
  max-height: 300px;
  padding: 20px;
  overflow-y: auto;

  .publish-info {
    margin-bottom: 12px;
    font-size: 12px;
    color: #999;
  }

  .content {
    line-height: 1.8;
    color: #333;
    white-space: pre-wrap;
  }

  .loading {
    padding: 40px;
    color: #999;
    text-align: center;
  }
}

.modal-footer {
  display: flex;
  gap: 12px;
  justify-content: flex-end;
  padding: 16px 20px;
  border-top: 1px solid #eee;
}

.fade-enter-active,
.fade-leave-active {
  transition: opacity 0.3s ease;
}

.fade-enter-from,
.fade-leave-to {
  opacity: 0;
}
</style>

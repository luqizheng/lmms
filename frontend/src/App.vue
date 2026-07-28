<template>
  <el-config-provider :locale="currentLocale">
    <router-view />
    <ReDialog />
    <ReDrawer />
    <ReAnnouncementModal
      :show="popoverVisible"
      :announcement-id="popoverId"
      @close="handlePopoverClose"
    />
  </el-config-provider>
</template>

<script lang="ts">
import { checkVersion } from "version-rocket";
import { ElConfigProvider } from "element-plus";
import { useRouter, useRoute } from "vue-router";
import { useGlobal, useWatermark } from "@pureadmin/utils";
import {
  defineComponent,
  computed,
  watch,
  nextTick,
  ref,
  onMounted
} from "vue";
import { ReDialog, closeAllDialog } from "@/components/ReDialog";
import { ReDrawer, closeAllDrawer } from "@/components/ReDrawer";
import ReAnnouncementModal from "@/components/ReAnnouncementModal";
import { usePopoverAnnouncement } from "@/hooks/usePopoverAnnouncement";
import en from "element-plus/es/locale/lang/en";
import zhCn from "element-plus/es/locale/lang/zh-cn";
import plusEn from "plus-pro-components/es/locale/lang/en";
import plusZhCn from "plus-pro-components/es/locale/lang/zh-cn";

export default defineComponent({
  name: "app",
  components: {
    [ElConfigProvider.name]: ElConfigProvider,
    ReDialog,
    ReDrawer,
    ReAnnouncementModal
  },
  setup() {
    const route = useRoute();
    const router = useRouter();
    const { setWatermark, clear } = useWatermark();
    const { $storage } = useGlobal<GlobalPropertiesApi>();
    const watermarkEnable = computed(() => $storage.configure?.watermark);
    const watermarkText = computed(() => $storage.configure?.watermarkText);
    const currentLocale = computed(() => {
      return $storage.locale?.locale === "zh"
        ? { ...zhCn, ...plusZhCn }
        : { ...en, ...plusEn };
    });

    const popover = usePopoverAnnouncement();
    const popoverLoaded = ref(false);

    const popoverVisible = popover.visible;
    const popoverId = popover.currentId;

    function handlePopoverClose() {
      popover.handleClose();
    }

    router.beforeEach((to, from) => {
      if (to.fullPath === from.fullPath) return;
      closeAllDialog();
      closeAllDrawer();
    });

    watch(
      [watermarkEnable, watermarkText, () => route.name],
      async ([enable, text, name]) => {
        await nextTick();
        if (enable && name !== "Login") {
          setWatermark(text, { verticalOffset: 170 });
        } else {
          clear();
        }
        if (name !== "Login" && !popoverLoaded.value) {
          popoverLoaded.value = true;
          popover.loadPopoverAnnouncements();
        }
      },
      {
        immediate: true
      }
    );

    onMounted(() => {
      if (route.name !== "Login" && !popoverLoaded.value) {
        popoverLoaded.value = true;
        popover.loadPopoverAnnouncements();
      }
    });

    return {
      currentLocale,
      popoverVisible,
      popoverId,
      handlePopoverClose
    };
  },
  beforeCreate() {
    const { version, name: title } = __APP_INFO__.pkg;
    const { VITE_PUBLIC_PATH, MODE } = import.meta.env;
    // https://github.com/guMcrey/version-rocket/blob/main/README.zh-CN.md#api
    if (MODE === "production") {
      // 版本实时更新检测，只作用于线上环境
      checkVersion(
        // config
        {
          // 5分钟检测一次版本
          pollingTime: 300000,
          localPackageVersion: version,
          originVersionFileUrl: `${location.origin}${VITE_PUBLIC_PATH}version.json`
        },
        // options
        {
          title,
          description: "检测到新版本",
          buttonText: "立即更新"
        }
      );
    }
  }
});
</script>

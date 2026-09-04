<script setup lang="ts">
import { ref, computed } from "vue";
import { useI18n } from "vue-i18n";
import ReCol from "@/components/ReCol";
import { formRules } from "./utils/rule";
import { FormProps } from "./utils/types";
import { transformI18n } from "@/plugins/i18n";
import { IconSelect } from "@/components/ReIcon";
import Segmented from "@/components/ReSegmented";
import ReAnimateSelector from "@/components/ReAnimateSelector";
import {
  menuTypeOptions,
  showLinkOptions,
  fixedTagOptions,
  keepAliveOptions,
  hiddenTagOptions,
  showParentOptions,
  frameLoadingOptions
} from "./utils/enums";

const { t } = useI18n();
const props = withDefaults(defineProps<FormProps>(), {
  formInline: () => ({
    menuType: 0,
    higherMenuOptions: [],
    parentId: 0,
    title: "",
    name: "",
    path: "",
    component: "",
    rank: 99,
    redirect: "",
    icon: "",
    extraIcon: "",
    enterTransition: "",
    leaveTransition: "",
    activePath: "",
    auths: "",
    frameSrc: "",
    frameLoading: true,
    keepAlive: false,
    hiddenTag: false,
    fixedTag: false,
    showLink: true,
    showParent: false,
    roles: []
  })
});

const ruleFormRef = ref();
const newFormInline = ref(props.formInline);
const roleOptions = computed(() => props.roleOptions ?? []);

function getRef() {
  return ruleFormRef.value;
}

defineExpose({ getRef });
</script>

<template>
  <el-form
    ref="ruleFormRef"
    :model="newFormInline"
    :rules="formRules"
    label-width="82px"
  >
    <el-row :gutter="30">
      <re-col>
        <el-form-item :label="t('menus.menuFormLabelType')">
          <Segmented
            v-model="newFormInline.menuType"
            :options="menuTypeOptions"
          />
        </el-form-item>
      </re-col>

      <re-col>
        <el-form-item :label="t('menus.menuFormLabelParent')">
          <el-cascader
            v-model="newFormInline.parentId"
            class="w-full"
            :options="newFormInline.higherMenuOptions"
            :props="{
              value: 'id',
              label: 'title',
              emitPath: false,
              checkStrictly: true
            }"
            clearable
            filterable
            :placeholder="t('menus.menuFormPlaceholderParent')"
          >
            <template #default="{ node, data }">
              <span>{{ transformI18n(data.title) }}</span>
              <span v-if="!node.isLeaf"> ({{ data.children.length }}) </span>
            </template>
          </el-cascader>
        </el-form-item>
      </re-col>

      <re-col :value="12" :xs="24" :sm="24">
        <el-form-item :label="t('menus.menuFormLabelTitle')" prop="title">
          <el-input
            v-model="newFormInline.title"
            clearable
            :placeholder="t('menus.menuFormPlaceholderTitle')"
          />
        </el-form-item>
      </re-col>
      <re-col v-if="newFormInline.menuType !== 3" :value="12" :xs="24" :sm="24">
        <el-form-item :label="t('menus.menuFormLabelRouteName')" prop="name">
          <el-input
            v-model="newFormInline.name"
            clearable
            :placeholder="t('menus.menuFormPlaceholderRouteName')"
          />
        </el-form-item>
      </re-col>

      <re-col v-if="newFormInline.menuType !== 3" :value="12" :xs="24" :sm="24">
        <el-form-item :label="t('menus.menuFormLabelPath')" prop="path">
          <el-input
            v-model="newFormInline.path"
            clearable
            :placeholder="t('menus.menuFormPlaceholderPath')"
          />
        </el-form-item>
      </re-col>
      <re-col
        v-show="newFormInline.menuType === 0"
        :value="12"
        :xs="24"
        :sm="24"
      >
        <el-form-item :label="t('menus.menuFormLabelComponent')">
          <el-input
            v-model="newFormInline.component"
            clearable
            :placeholder="t('menus.menuFormPlaceholderComponent')"
          />
        </el-form-item>
      </re-col>

      <re-col :value="12" :xs="24" :sm="24">
        <el-form-item :label="t('menus.menuFormLabelRank')">
          <el-input-number
            v-model="newFormInline.rank"
            class="w-full!"
            :min="1"
            :max="9999"
            controls-position="right"
          />
        </el-form-item>
      </re-col>
      <re-col
        v-show="newFormInline.menuType === 0"
        :value="12"
        :xs="24"
        :sm="24"
      >
        <el-form-item :label="t('menus.menuFormLabelRedirect')">
          <el-input
            v-model="newFormInline.redirect"
            clearable
            :placeholder="t('menus.menuFormPlaceholderRedirect')"
          />
        </el-form-item>
      </re-col>

      <re-col
        v-show="newFormInline.menuType !== 3"
        :value="12"
        :xs="24"
        :sm="24"
      >
        <el-form-item :label="t('menus.menuFormLabelIcon')">
          <IconSelect v-model="newFormInline.icon" class="w-full" />
        </el-form-item>
      </re-col>
      <re-col
        v-show="newFormInline.menuType !== 3"
        :value="12"
        :xs="24"
        :sm="24"
      >
        <el-form-item :label="t('menus.menuFormLabelExtraIcon')">
          <el-input
            v-model="newFormInline.extraIcon"
            clearable
            :placeholder="t('menus.menuFormPlaceholderExtraIcon')"
          />
        </el-form-item>
      </re-col>

      <re-col v-show="newFormInline.menuType < 2" :value="12" :xs="24" :sm="24">
        <el-form-item :label="t('menus.menuFormLabelEnter')">
          <ReAnimateSelector
            v-model="newFormInline.enterTransition"
            :placeholder="t('menus.menuFormPlaceholderEnter')"
          />
        </el-form-item>
      </re-col>
      <re-col v-show="newFormInline.menuType < 2" :value="12" :xs="24" :sm="24">
        <el-form-item :label="t('menus.menuFormLabelLeave')">
          <ReAnimateSelector
            v-model="newFormInline.leaveTransition"
            :placeholder="t('menus.menuFormPlaceholderLeave')"
          />
        </el-form-item>
      </re-col>

      <re-col
        v-show="newFormInline.menuType === 0"
        :value="12"
        :xs="24"
        :sm="24"
      >
        <el-form-item :label="t('menus.menuFormLabelActive')">
          <el-input
            v-model="newFormInline.activePath"
            clearable
            :placeholder="t('menus.menuFormPlaceholderActive')"
          />
        </el-form-item>
      </re-col>
      <re-col v-if="newFormInline.menuType === 3" :value="12" :xs="24" :sm="24">
        <el-form-item :label="t('menus.menuFormLabelAuths')" prop="auths">
          <el-input
            v-model="newFormInline.auths"
            clearable
            :placeholder="t('menus.menuFormPlaceholderAuths')"
          />
        </el-form-item>
      </re-col>

      <re-col
        v-show="newFormInline.menuType === 1"
        :value="12"
        :xs="24"
        :sm="24"
      >
        <el-form-item :label="t('menus.menuFormLabelFrameSrc')">
          <el-input
            v-model="newFormInline.frameSrc"
            clearable
            :placeholder="t('menus.menuFormPlaceholderFrameSrc')"
          />
        </el-form-item>
      </re-col>
      <re-col v-if="newFormInline.menuType === 1" :value="12" :xs="24" :sm="24">
        <el-form-item :label="t('menus.menuFormLabelFrameLoading')">
          <Segmented
            :modelValue="newFormInline.frameLoading ? 0 : 1"
            :options="frameLoadingOptions"
            @change="
              ({ option: { value } }) => {
                newFormInline.frameLoading = value;
              }
            "
          />
        </el-form-item>
      </re-col>

      <re-col
        v-show="newFormInline.menuType !== 3"
        :value="12"
        :xs="24"
        :sm="24"
      >
        <el-form-item label="可见角色">
          <el-select
            v-model="newFormInline.roles"
            class="w-full"
            multiple
            filterable
            allow-create
            default-first-option
            clearable
            placeholder="留空 = 所有人可见"
          >
            <el-option
              v-for="r in roleOptions"
              :key="r.value"
              :label="r.label"
              :value="r.value"
            />
          </el-select>
        </el-form-item>
      </re-col>

      <re-col
        v-show="newFormInline.menuType !== 3"
        :value="12"
        :xs="24"
        :sm="24"
      >
        <el-form-item :label="t('menus.menuFormLabelShowLink')">
          <Segmented
            :modelValue="newFormInline.showLink ? 0 : 1"
            :options="showLinkOptions"
            @change="
              ({ option: { value } }) => {
                newFormInline.showLink = value;
              }
            "
          />
        </el-form-item>
      </re-col>
      <re-col
        v-show="newFormInline.menuType !== 3"
        :value="12"
        :xs="24"
        :sm="24"
      >
        <el-form-item :label="t('menus.menuFormLabelShowParent')">
          <Segmented
            :modelValue="newFormInline.showParent ? 0 : 1"
            :options="showParentOptions"
            @change="
              ({ option: { value } }) => {
                newFormInline.showParent = value;
              }
            "
          />
        </el-form-item>
      </re-col>

      <re-col v-show="newFormInline.menuType < 2" :value="12" :xs="24" :sm="24">
        <el-form-item :label="t('menus.menuFormLabelKeepAlive')">
          <Segmented
            :modelValue="newFormInline.keepAlive ? 0 : 1"
            :options="keepAliveOptions"
            @change="
              ({ option: { value } }) => {
                newFormInline.keepAlive = value;
              }
            "
          />
        </el-form-item>
      </re-col>

      <re-col v-show="newFormInline.menuType < 2" :value="12" :xs="24" :sm="24">
        <el-form-item :label="t('menus.menuFormLabelHiddenTag')">
          <Segmented
            :modelValue="newFormInline.hiddenTag ? 1 : 0"
            :options="hiddenTagOptions"
            @change="
              ({ option: { value } }) => {
                newFormInline.hiddenTag = value;
              }
            "
          />
        </el-form-item>
      </re-col>
      <re-col v-show="newFormInline.menuType < 2" :value="12" :xs="24" :sm="24">
        <el-form-item :label="t('menus.menuFormLabelFixedTag')">
          <Segmented
            :modelValue="newFormInline.fixedTag ? 0 : 1"
            :options="fixedTagOptions"
            @change="
              ({ option: { value } }) => {
                newFormInline.fixedTag = value;
              }
            "
          />
        </el-form-item>
      </re-col>
    </el-row>
  </el-form>
</template>

<script setup lang="ts">
import { computed, onBeforeUnmount, ref, watch } from "vue";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Select } from "@/components/ui/select";
import { Skeleton } from "@/components/ui/skeleton";
import { Pagination } from "@/components/ui/pagination";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow
} from "@/components/ui/table";
import { useI18n } from "vue-i18n";

export interface TableColumn {
  /** 列标题 */
  label: string;
  /** 数据字段 */
  prop: string;
  /** 数值列：等宽数字右对齐 */
  numeric?: boolean;
  /** 表头是否可排序 */
  sortable?: boolean;
  /** 自定义单元格渲染 */
  slot?: string;
}

export interface QueryField {
  /** 字段名 */
  prop: string;
  /** 输入框占位 */
  placeholder?: string;
  /** 控件类型 */
  type?: "text" | "select";
  /** select 的选项 */
  options?: Array<{ label: string; value: string }>;
}

interface Props {
  /** 列定义 */
  columns: TableColumn[];
  /** 数据行 */
  rows: Record<string, unknown>[];
  /** 查询条件 */
  query?: QueryField[];
  /** 是否加载中（>300ms 才显示骨架） */
  loading?: boolean;
  /** 空态文案 */
  emptyText?: string;
  /** 分页 */
  page?: number;
  pageSize?: number;
  total?: number;
  /** 表格标题（无障碍用） */
  caption?: string;
  /** 可选的每页条数档位；给了就渲染切换器 */
  pageSizes?: number[];
}

const { t } = useI18n();

const props = withDefaults(defineProps<Props>(), {
  query: () => [],
  loading: false,
  emptyText: undefined,
  page: 1,
  pageSize: 10,
  total: 0,
  caption: undefined,
  pageSizes: () => []
});

const emit = defineEmits<{
  search: [params: Record<string, string>];
  "update:page": [page: number];
  sort: [prop: string, order: "ascending" | "descending"];
  "update:pageSize": [pageSize: number];
}>();

const keywords = ref<Record<string, string>>({});
const selectValues = ref<Record<string, string>>({});

const totalPages = computed(() =>
  Math.max(1, Math.ceil(props.total / props.pageSize))
);

/** 当前排序列与方向（Phase 2 §5.3：排序状态必须通过 aria-sort 暴露） */
/** 骨架屏出现阈值 300ms（Phase 2 §7.1）：请求更快返回时不闪骨架 */
const SKELETON_DELAY = 300;
const showSkeleton = ref(false);
let skeletonTimer: ReturnType<typeof setTimeout> | undefined;

watch(
  () => props.loading,
  loading => {
    clearTimeout(skeletonTimer);
    skeletonTimer = undefined;
    if (!loading) {
      showSkeleton.value = false;
      return;
    }
    skeletonTimer = setTimeout(() => {
      showSkeleton.value = true;
    }, SKELETON_DELAY);
  },
  { immediate: true }
);

onBeforeUnmount(() => clearTimeout(skeletonTimer));

const sortProp = ref<string | undefined>(undefined);
const sortOrder = ref<"ascending" | "descending">("ascending");

function sortOf(
  column: TableColumn
): "none" | "ascending" | "descending" | undefined {
  if (!column.sortable) return undefined;
  if (sortProp.value !== column.prop) return "none";
  return sortOrder.value;
}

function onSearch() {
  emit("search", { ...keywords.value, ...selectValues.value });
}

function onPageSizeChange(event: Event) {
  const size = Number((event.target as HTMLSelectElement).value);
  if (Number.isFinite(size) && size !== props.pageSize) {
    emit("update:pageSize", size);
  }
}

function onSort(column: TableColumn) {
  if (!column.sortable) return;
  if (sortProp.value === column.prop) {
    sortOrder.value =
      sortOrder.value === "ascending" ? "descending" : "ascending";
  } else {
    sortProp.value = column.prop;
    sortOrder.value = "ascending";
  }
  emit("sort", column.prop, sortOrder.value);
}
</script>

<template>
  <section class="ui-data-table">
    <form
      v-if="props.query.length"
      class="ui-data-table__query"
      @submit.prevent="onSearch"
    >
      <div
        v-for="field in props.query"
        :key="field.prop"
        class="ui-data-table__field"
      >
        <template v-if="field.type === 'select'">
          <Select
            :model-value="selectValues[field.prop] ?? ''"
            :options="field.options ?? []"
            :placeholder="field.placeholder"
            :aria-label="field.placeholder"
            @update:model-value="
              value => (selectValues[field.prop] = value as string)
            "
          />
        </template>
        <Input
          v-else
          v-model="keywords[field.prop]"
          :placeholder="field.placeholder"
          :aria-label="field.placeholder"
        />
      </div>
      <Button type="submit">{{ t("system.common.search") }}</Button>
    </form>

    <Table :caption="props.caption">
      <TableHead>
        <TableRow>
          <TableHeader
            v-for="column in props.columns"
            :key="column.prop"
            :sortable="column.sortable"
            :sort="sortOf(column)"
            @click="onSort(column)"
          >
            {{ column.label }}
          </TableHeader>
        </TableRow>
      </TableHead>
      <TableBody>
        <template v-if="showSkeleton">
          <TableRow v-for="line in 6" :key="`skeleton-${line}`">
            <TableCell v-for="column in props.columns" :key="column.prop">
              <Skeleton />
            </TableCell>
          </TableRow>
        </template>
        <TableRow v-else-if="!props.rows.length">
          <TableCell
            :colspan="props.columns.length"
            class="ui-data-table__empty"
          >
            {{ props.emptyText ?? t("system.common.empty") }}
          </TableCell>
        </TableRow>
        <TableRow v-for="(row, index) in props.rows" v-else :key="index">
          <TableCell
            v-for="column in props.columns"
            :key="column.prop"
            :numeric="column.numeric"
          >
            <slot
              :name="column.slot ?? column.prop"
              :row="row"
              :value="row[column.prop]"
            >
              {{ row[column.prop] ?? "-" }}
            </slot>
          </TableCell>
        </TableRow>
      </TableBody>
    </Table>

    <!-- 窄屏（<640）表格卡片化：Phase 2 §4.2 的结论，无第二方案 -->
    <ul class="ui-data-table__cards">
      <template v-if="showSkeleton">
        <li
          v-for="line in 6"
          :key="`skeleton-${line}`"
          class="ui-data-table__card"
        >
          <Skeleton />
        </li>
      </template>
      <li
        v-else-if="!props.rows.length"
        class="ui-data-table__card ui-data-table__card--empty"
      >
        {{ props.emptyText ?? t("system.common.empty") }}
      </li>
      <template v-else>
        <li
          v-for="(row, index) in props.rows"
          :key="index"
          class="ui-data-table__card"
        >
          <div
            v-for="column in props.columns"
            :key="column.prop"
            class="ui-data-table__card-field"
          >
            <span class="ui-data-table__card-label">{{ column.label }}</span>
            <span
              class="ui-data-table__card-value"
              :class="{ 'is-numeric': column.numeric }"
            >
              <slot
                :name="column.slot ?? column.prop"
                :row="row"
                :value="row[column.prop]"
              >
                {{ row[column.prop] ?? "-" }}
              </slot>
            </span>
          </div>
        </li>
      </template>
    </ul>

    <footer class="ui-data-table__footer">
      <div v-if="props.pageSizes.length" class="ui-data-table__size">
        <label class="ui-data-table__size-label" for="ui-data-table-size">
          {{ t("common.pageSize") }}
        </label>
        <select
          id="ui-data-table-size"
          class="ui-data-table__size-select"
          :value="props.pageSize"
          @change="onPageSizeChange"
        >
          <option v-for="size in props.pageSizes" :key="size" :value="size">
            {{ size }}
          </option>
        </select>
      </div>
      <span class="ui-data-table__count">{{
        t("system.common.total", { count: props.total })
      }}</span>
      <Pagination
        :page="props.page"
        :total="props.total"
        :page-size="props.pageSize"
        @update:page="emit('update:page', $event)"
      />
    </footer>
  </section>
</template>

<style scoped>
.ui-data-table {
  display: flex;
  flex-direction: column;
  gap: var(--space-4);
}

.ui-data-table__query {
  display: flex;
  flex-wrap: wrap;
  align-items: flex-end;
  gap: var(--space-3);
}

.ui-data-table__field {
  width: 220px;
}

.ui-data-table__cards {
  display: none;
  flex-direction: column;
  gap: var(--space-3);
  margin: 0;
  padding: 0;
  list-style: none;
}

.ui-data-table__card {
  padding: var(--space-4);
  background: var(--surface-raised);
  border: 1px solid var(--border-default);
  border-radius: var(--radius-lg);
  box-shadow: var(--shadow-xs);
}

.ui-data-table__card--empty {
  text-align: center;
  color: var(--text-tertiary);
}

.ui-data-table__card-field {
  display: flex;
  align-items: baseline;
  justify-content: space-between;
  gap: var(--space-4);
  padding: var(--space-2) 0;
}

.ui-data-table__card-label {
  flex: none;
  font-family: var(--font-ui);
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
  color: var(--text-tertiary);
}

.ui-data-table__card-value {
  text-align: right;
  color: var(--text-primary);
}

.ui-data-table__card-value.is-numeric {
  font-family: var(--font-mono);
  font-variant-numeric: tabular-nums;
}

/* <640：表格换卡片（Phase 2 §4.2） */
@media (max-width: 639px) {
  .ui-data-table__cards {
    display: flex;
  }

  .ui-data-table :deep(.ui-table__wrapper) {
    display: none;
  }
}

.ui-data-table__empty {
  padding: var(--space-10) 0;
  text-align: center;
  color: var(--text-tertiary);
}

.ui-data-table__footer {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-4);
}

.ui-data-table__size {
  display: inline-flex;
  align-items: center;
  gap: var(--space-2);
}

.ui-data-table__size-label {
  font-family: var(--font-ui);
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
  color: var(--text-tertiary);
}

.ui-data-table__size-select {
  height: var(--size-control-sm);
  padding: 0 var(--space-2);
  border: 1px solid var(--border-default);
  border-radius: var(--radius-sm);
  background: var(--surface-raised);
  font-family: var(--font-mono);
  font-size: var(--font-size-caption);
  color: var(--text-primary);
}

.ui-data-table__count {
  font-family: var(--font-ui);
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
  color: var(--text-tertiary);
}
</style>

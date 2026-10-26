<script setup lang="ts">
import { computed, ref } from "vue";
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
}

const props = withDefaults(defineProps<Props>(), {
  query: () => [],
  loading: false,
  emptyText: "还没有记录",
  page: 1,
  pageSize: 10,
  total: 0,
  caption: undefined
});

const emit = defineEmits<{
  search: [params: Record<string, string>];
  "update:page": [page: number];
  sort: [prop: string];
}>();

const keywords = ref<Record<string, string>>({});
const selectValues = ref<Record<string, string>>({});

const totalPages = computed(() =>
  Math.max(1, Math.ceil(props.total / props.pageSize))
);

function onSearch() {
  emit("search", { ...keywords.value, ...selectValues.value });
}

function onSort(column: TableColumn) {
  if (column.sortable) emit("sort", column.prop);
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
            @update:model-value="
              value => (selectValues[field.prop] = value as string)
            "
          />
        </template>
        <Input
          v-else
          v-model="keywords[field.prop]"
          :placeholder="field.placeholder"
        />
      </div>
      <Button type="submit">查询</Button>
    </form>

    <Table :caption="props.caption">
      <TableHead>
        <TableRow>
          <TableHeader
            v-for="column in props.columns"
            :key="column.prop"
            :sortable="column.sortable"
            @click="onSort(column)"
          >
            {{ column.label }}
          </TableHeader>
        </TableRow>
      </TableHead>
      <TableBody>
        <TableRow v-if="props.loading">
          <TableCell v-for="column in props.columns" :key="column.prop">
            <Skeleton />
          </TableCell>
        </TableRow>
        <TableRow v-else-if="!props.rows.length">
          <TableCell
            :colspan="props.columns.length"
            class="ui-data-table__empty"
          >
            {{ props.emptyText }}
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

    <footer class="ui-data-table__footer">
      <span class="ui-data-table__count">共 {{ props.total }} 条</span>
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

.ui-data-table__count {
  font-family: var(--font-ui);
  font-size: var(--font-size-caption);
  line-height: var(--line-height-caption);
  color: var(--text-tertiary);
}
</style>

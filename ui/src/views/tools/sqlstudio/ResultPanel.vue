<script setup lang="ts">
import { computed, ref } from 'vue';
import { NDataTable, NButton, NIcon } from 'naive-ui';
import type { DataTableInst } from 'naive-ui';
import { useSqlStudioStore } from '@/store/modules/sqlstudio';
import { DownloadOutline } from '@vicons/ionicons5';

const store = useSqlStudioStore();
const tableRef = ref<DataTableInst | null>(null);

const pagination = computed(() => ({
  page: store.pagination.page,
  pageSize: store.pagination.pageSize,
  itemCount: store.pagination.total,
  showSizePicker: true,
  pageSizes: [10, 20, 50, 100],
  prefix: ({ itemCount }: { itemCount: number }) => `共 ${itemCount} 条`,
  onChange: (page: number) => store.handlePageChange(page),
  onUpdatePageSize: (pageSize: number) => store.handlePageSizeChange(pageSize)
}));

function handleSorterChange(sorter: any) {
  store.handleSorterChange(sorter);
}

function exportCsv() {
  tableRef.value?.downloadCsv({ fileName: 'export-data' });
}
</script>

<template>
  <div class="h-full flex flex-col p-2 gap-2">
    <div class="flex justify-between items-center shrink-0">
      <div class="text-xs text-gray-500">
        <span v-if="store.executionResult">
          {{ store.executionResult.message }}
          <span v-if="store.executionResult.affected_rows !== null && store.executionResult.affected_rows !== undefined">
            | Affected Rows: {{ store.executionResult.affected_rows }}
          </span>
          <span v-if="store.executionResult.execution_time_ms !== undefined">
            | Time: {{ store.executionResult.execution_time_ms }}ms
          </span>
        </span>
      </div>
      <NButton size="small" @click="exportCsv" :disabled="!store.tableData.length">
        <template #icon>
          <NIcon><DownloadOutline /></NIcon>
        </template>
        导出CSV
      </NButton>
    </div>
    <div class="flex-1 overflow-hidden relative">
      <NDataTable
        v-if="store.tableData.length > 0 || store.columns.length > 0"
        ref="tableRef"
        remote
        :columns="store.columns"
        :data="store.tableData"
        :loading="store.loading"
        :pagination="pagination"
        :bordered="true"
        size="small"
        striped
        flex-height
        class="h-full"
        @update:sorter="handleSorterChange"
      />
      <div v-else-if="store.executionResult && !store.loading" class="h-full flex items-center justify-center text-gray-400">
        <div class="text-center">
          <p class="text-lg">{{ store.executionResult.message }}</p>
          <p v-if="store.executionResult.affected_rows !== null && store.executionResult.affected_rows !== undefined">
            Affected Rows: {{ store.executionResult.affected_rows }}
          </p>
        </div>
      </div>
      <div v-else-if="!store.loading" class="h-full flex items-center justify-center text-gray-400">
        暂无数据
      </div>
    </div>
  </div>
</template>

<style scoped>
:deep(.n-data-table .n-data-table-th) {
  padding: 8px;
}
:deep(.n-data-table .n-data-table-td) {
  padding: 8px;
}
</style>

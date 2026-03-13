<script setup lang="ts">
import { h, ref } from 'vue';
import { NDataTable } from 'naive-ui';
import type {
  DataTableColumns,
  DataTableGetCsvCell,
  DataTableGetCsvHeader,
  DataTableInst,
  DataTableRowData
} from 'naive-ui';

interface Song {
  key: number;
  name: string;
  age: number;
  address: string;
}

const columns: DataTableColumns<DataTableRowData> = [
  {
    title: 'Name',
    key: 'name',
    sorter: 'default',
    render(rowData) {
      return h('span', { style: { color: 'blue' } }, rowData.name);
    }
  },
  {
    title: () => h('span', { style: { color: 'red' } }, 'Age'),
    key: 'age',
    sorter: (row1: object, row2: object) => (row1 as Song).age - (row2 as Song).age
  },
  {
    title: 'Address',
    key: 'address',
    filterOptions: [
      {
        label: 'London',
        value: 'London'
      },
      {
        label: 'New York',
        value: 'New York'
      }
    ],
    filter: (value: string | number, row: object) => {
      const keyword = String(value);
      return (row as Song).address.includes(keyword);
    }
  }
];

const data: Song[] = [
  {
    key: 0,
    name: 'John Brown',
    age: 18,
    address: 'New York No. 1 Lake Park'
  },
  {
    key: 1,
    name: 'Jim Green',
    age: 28,
    address: 'London No. 1 Lake Park'
  },
  {
    key: 2,
    name: 'Joe Black',
    age: 38,
    address: 'Sidney No. 1 Lake Park'
  },
  {
    key: 3,
    name: 'Jim Red',
    age: 48,
    address: 'London No. 2 Lake Park'
  }
];

const tableRef = ref<DataTableInst>();

function downloadCsv() {
  return tableRef.value?.downloadCsv({ fileName: 'data-table' });
}

function exportSorterAndFilterCsv() {
  return tableRef.value?.downloadCsv({
    fileName: 'sorter-filter',
    keepOriginalData: false
  });
}

const getCsvCell: DataTableGetCsvCell = (value, _, column) => {
  if (column.key === 'age') {
    return `${value} years old`;
  }
  return value;
};

const getCsvHeader: DataTableGetCsvHeader = col => {
  if (typeof col.title === 'function') {
    return col.key === 'age' ? 'Age' : 'Unknown';
  }
  return col.title || 'Unknown';
};

const pagination = false as const;
</script>

<template>
  <div class="p-1">
    <NSpace vertical :size="5">
      <NSpace :size="5">
        <NButtonGroup size="small">
          <NButton @click="downloadCsv">按钮1</NButton>
          <NButton @click="exportSorterAndFilterCsv">按钮2</NButton>
        </NButtonGroup>
      </NSpace>
      <NDataTable
        ref="tableRef"
        :columns="columns"
        :data="data"
        :pagination="pagination"
        :bordered="true"
        :get-csv-cell="getCsvCell"
        :get-csv-header="getCsvHeader"
        size="small"
        :striped="true"
      />
    </NSpace>
  </div>
</template>

<style scoped>
:deep(.n-data-table .n-data-table-th) {
  padding: 4px 4px;
}
:deep(.n-data-table .n-data-table-td) {
  padding: 4px 4px;
}
</style>

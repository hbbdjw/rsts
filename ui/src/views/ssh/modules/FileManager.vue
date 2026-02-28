<script setup lang="ts">
import { computed, h, onMounted, reactive, ref } from 'vue';
import {
  NBreadcrumb,
  NBreadcrumbItem,
  NButton,
  NDataTable,
  NForm,
  NFormItem,
  NIcon,
  NInput,
  NInputNumber,
  NModal,
  NPopconfirm,
  NSpace,
  NUpload,
  useMessage
} from 'naive-ui';
import type { DataTableColumns, UploadCustomRequestOptions } from 'naive-ui';
import dayjs from 'dayjs';
import {
  ArrowUpOutline,
  CloudUploadOutline,
  DocumentOutline,
  FolderOpenOutline,
  FolderOutline,
  RefreshOutline
} from '@vicons/ionicons5';
import {
  createDir,
  createSftpSession,
  deleteFile,
  downloadFile,
  listFiles,
  readFile,
  renameFile,
  setPermissions,
  uploadFile,
  writeFile
} from '@/service/api/sftp';
import type { FileEntry } from '@/service/api/sftp';
import TextEditor from '@/components/text-editor/index.vue';

const props = defineProps<{
  hostname?: string;
  port?: number;
  username?: string;
  password?: string;
  autoConnect?: boolean;
}>();

const emit = defineEmits<{
  (e: 'connect', form: any): void;
}>();

const message = useMessage();
const loading = ref(false);
const sessionId = ref<number | null>(null);
const currentPath = ref('/');
const files = ref<FileEntry[]>([]);
const showConnectModal = ref(false);

const connectForm = reactive({
  hostname: props.hostname || '',
  port: props.port || 22,
  username: props.username || '',
  password: props.password || ''
});

// Path navigation
const pathItems = computed(() => {
  if (currentPath.value === '/') return ['/'];
  const parts = currentPath.value.split('/').filter(Boolean);
  return ['/', ...parts];
});

function handlePathClick(index: number) {
  const previousPath = currentPath.value;
  if (index === 0) {
    currentPath.value = '/';
  } else {
    // Reconstruct path from parts up to index
    // index 0 is root '/', index 1 is first folder
    const parts = currentPath.value.split('/').filter(Boolean);
    const selectedParts = parts.slice(0, index);
    currentPath.value = `/${selectedParts.join('/')}`;
  }
  if (currentPath.value !== previousPath) {
    loadFiles();
  }
}

// Modal states
const showMkdirModal = ref(false);
const showChmodModal = ref(false);
const mkdirForm = reactive({ name: '' });
const chmodForm = reactive({ path: '', mode: 755 });

// Inline rename state
const editingFileName = ref<string | null>(null);
const editingInputValue = ref('');

// Text Editor state
const showEditor = ref(false);
const editorContent = ref('');
const editorFilename = ref('');
const editorPath = ref('');
const editorLoading = ref(false);

// Columns definition
const columns: DataTableColumns<FileEntry> = [
  {
    title: '',
    key: 'icon',
    width: 40,
    render(row) {
      return h(NIcon, { size: 20 }, { default: () => (row.is_dir ? h(FolderOutline) : h(DocumentOutline)) });
    }
  },
  {
    title: '',
    key: 'name',
    sorter: 'default',
    render(row) {
      if (editingFileName.value === row.name) {
        return h(NInput, {
          value: editingInputValue.value,
          onUpdateValue: v => (editingInputValue.value = v),
          onBlur: handleRenameConfirm,
          onKeydown: (e: KeyboardEvent) => {
            if (e.key === 'Enter') {
              handleRenameConfirm();
            } else if (e.key === 'Escape') {
              editingFileName.value = null;
            }
          },
          autofocus: true,
          size: 'tiny'
        });
      }
      return h(
        'span',
        {
          style: { color: row.is_dir ? '#1890ff' : 'inherit' }
        },
        row.name
      );
    }
  },
  {
    title: '',
    key: 'size',
    width: 100,
    render(row) {
      if (row.is_dir) return '-';
      const units = ['B', 'KB', 'MB', 'GB'];
      let size = row.size;
      let unitIndex = 0;
      while (size >= 1024 && unitIndex < units.length - 1) {
        size /= 1024;
        unitIndex += 1;
      }
      return `${size.toFixed(1)} ${units[unitIndex]}`;
    }
  },
  {
    title: '',
    key: 'mtime',
    width: 160,
    render(row) {
      return dayjs.unix(row.mtime).format('YYYY-MM-DD HH:mm:ss');
    }
  },
  {
    title: '',
    key: 'permissions',
    width: 100,
    render(row) {
      return row.permissions.toString(8).slice(-3);
    }
  },
  {
    title: '',
    key: 'actions',
    width: 250,
    render(row) {
      return h(
        NSpace,
        {},
        {
          default: () => [
            h(
              NButton,
              { size: 'tiny', onClick: () => handleDownload(row), disabled: row.is_dir },
              { default: () => '下载' }
            ),
            h(NButton, { size: 'tiny', onClick: () => startRename(row) }, { default: () => '重命名' }),
            h(NButton, { size: 'tiny', onClick: () => openChmodModal(row) }, { default: () => '权限' }),
            h(
              NPopconfirm,
              { onPositiveClick: () => handleDelete(row) },
              {
                default: () => '确认删除？',
                trigger: () => h(NButton, { size: 'tiny', type: 'error' }, { default: () => '删除' })
              }
            )
          ]
        }
      );
    }
  }
];

// Row properties
const rowProps = (row: FileEntry) => {
  return {
    onDblclick: async () => {
      if (row.is_dir) {
        handleEnterDir(row);
      } else {
        await handleOpenFile(row);
      }
    },
    style: {
      cursor: 'pointer'
    }
  };
};

async function handleOpenFile(row: FileEntry) {
  if (sessionId.value === null) return;

  const path = currentPath.value === '/' ? `/${row.name}` : `${currentPath.value}/${row.name}`;

  loading.value = true;
  const { data, error } = await readFile({ session_id: sessionId.value, path });
  loading.value = false;

  if (error) {
    message.error(`读取文件失败: ${error.message}`);
    return;
  }

  if (data) {
    editorContent.value = data.content;
    editorFilename.value = row.name;
    editorPath.value = path;
    showEditor.value = true;
  }
}

async function handleEditorSave(content: string) {
  if (sessionId.value === null || !editorPath.value) return;

  editorLoading.value = true;
  const { error } = await writeFile({
    sessionId: sessionId.value,
    path: editorPath.value,
    content
  });
  editorLoading.value = false;

  if (error) {
    message.error(`保存失败: ${error.message}`);
  } else {
    message.success('保存成功');
    showEditor.value = false;
    loadFiles(); // Refresh to update timestamp/size if needed
  }
}

// Connection
async function connect(form?: any) {
  if (form) {
    Object.assign(connectForm, form);
  }
  if (!connectForm.hostname || !connectForm.username) {
    // showConnectModal.value = true;
    return;
  }

  loading.value = true;
  files.value = []; // Clear files on new connection attempt
  const { data, error } = await createSftpSession(connectForm);
  if (error) {
    message.error(`连接失败: ${error.message}`);
    showConnectModal.value = true;
    loading.value = false;
    return;
  }

  if (data) {
    sessionId.value = data.session_id;
    message.success('连接成功');
    emit('connect', { ...connectForm });
    showConnectModal.value = false;
    loadFiles();
  }
}

// File Operations
async function loadFiles() {
  if (sessionId.value === null) return;
  loading.value = true;

  const { data, error } = await listFiles({ session_id: sessionId.value, path: currentPath.value });

  if (error) {
    message.error(`加载文件列表失败: ${error.message}`);
    loading.value = false;
    return;
  }

  if (data) {
    // Filter out empty names and sort directories first
    files.value = data
      .filter(item => item.name && item.name.trim() !== '')
      .sort((a, b) => {
        if (a.is_dir && !b.is_dir) return -1;
        if (!a.is_dir && b.is_dir) return 1;
        return a.name.localeCompare(b.name);
      });
  }
  loading.value = false;
}

function handleEnterDir(row: FileEntry) {
  if (!row.is_dir) return;

  let newPath = '';
  if (row.path) {
    newPath = row.path;
  } else {
    // Fallback if path is not available (shouldn't happen with current API)
    newPath = currentPath.value === '/' ? `/${row.name}` : `${currentPath.value}/${row.name}`;
  }

  currentPath.value = newPath;
  loadFiles();
}

function handleGoUp() {
  if (currentPath.value === '/') return;
  const parts = currentPath.value.split('/');
  parts.pop();
  currentPath.value = parts.join('/') || '/';
  loadFiles();
}

function handleRefresh() {
  loadFiles();
}

// Download
function handleDownload(row: FileEntry) {
  if (sessionId.value === null) return;
  const path = currentPath.value === '/' ? `/${row.name}` : `${currentPath.value}/${row.name}`;
  downloadFile({ session_id: sessionId.value, path });
}

// Delete
async function handleDelete(row: FileEntry) {
  if (sessionId.value === null) return;
  const path = currentPath.value === '/' ? `/${row.name}` : `${currentPath.value}/${row.name}`;

  const { error } = await deleteFile({ session_id: sessionId.value, path });
  if (error) {
    message.error(`删除失败: ${error.message}`);
  } else {
    message.success('删除成功');
    loadFiles();
  }
}

// Rename
function startRename(row: FileEntry) {
  editingFileName.value = row.name;
  editingInputValue.value = row.name;
}

async function handleRenameConfirm() {
  if (sessionId.value === null || !editingFileName.value || !editingInputValue.value) return;

  const oldName = editingFileName.value;
  const newName = editingInputValue.value;

  if (oldName === newName) {
    editingFileName.value = null;
    return;
  }

  const path = currentPath.value === '/' ? `/${oldName}` : `${currentPath.value}/${oldName}`;

  const { error } = await renameFile({
    session_id: sessionId.value,
    path,
    new_name: newName
  });

  if (error) {
    message.error(`重命名失败: ${error.message}`);
  } else {
    message.success('重命名成功');
    editingFileName.value = null;
    loadFiles();
  }
}

// Mkdir
async function handleMkdir() {
  if (sessionId.value === null || !mkdirForm.name) return;
  const path = currentPath.value === '/' ? `/${mkdirForm.name}` : `${currentPath.value}/${mkdirForm.name}`;

  const { error } = await createDir({ session_id: sessionId.value, path });

  if (error) {
    message.error(`创建目录失败: ${error.message}`);
  } else {
    message.success('创建目录成功');
    showMkdirModal.value = false;
    mkdirForm.name = '';
    loadFiles();
  }
}

// Chmod
function openChmodModal(row: FileEntry) {
  const path = currentPath.value === '/' ? `/${row.name}` : `${currentPath.value}/${row.name}`;
  chmodForm.path = path;
  chmodForm.mode = Number.parseInt(row.permissions.toString(8).slice(-3), 8);
  showChmodModal.value = true;
}

async function handleChmod() {
  if (sessionId.value === null) return;

  // Convert decimal input (e.g. 755) to octal value (e.g. 493)
  const modeStr = chmodForm.mode.toString();
  const mode = Number.parseInt(modeStr, 8);

  const { error } = await setPermissions({ session_id: sessionId.value, path: chmodForm.path, mode });

  if (error) {
    message.error(`修改权限失败: ${error.message}`);
  } else {
    message.success('修改权限成功');
    showChmodModal.value = false;
    loadFiles();
  }
}

// Exposed methods for external control
function navigateTo(path: string) {
  if (!path) return;

  let targetPath = path.trim();

  // Handle '..' specifically
  if (targetPath === '..') {
    handleGoUp();
    return;
  }

  if (targetPath === '.') {
    return;
  }

  // Handle ~ as root (simplification since we don't know HOME)
  if (targetPath === '~') {
    currentPath.value = '/';
    loadFiles();
    return;
  }

  if (targetPath.startsWith('~/')) {
    targetPath = `/${targetPath.substring(2)}`;
  }

  // Handle absolute path
  if (targetPath.startsWith('/')) {
    currentPath.value = targetPath;
  } else {
    // Handle relative path
    const separator = currentPath.value === '/' ? '' : '/';
    currentPath.value = `${currentPath.value}${separator}${targetPath}`;
  }

  loadFiles();
}

defineExpose({
  connect,
  navigateTo
});
const uploadProgress = ref(0);
const uploadSpeed = ref('');
const isUploading = ref(false);

// Upload
async function handleUpload({ file }: UploadCustomRequestOptions) {
  if (sessionId.value === null || !file.file) return;

  const reader = new FileReader();
  reader.readAsDataURL(file.file);
  reader.onload = async () => {
    isUploading.value = true;
    uploadProgress.value = 0;
    uploadSpeed.value = '0 KB/s';

    const base64 = (reader.result as string).split(',')[1];

    try {
      const { error } = await uploadFile(
        {
          session_id: sessionId.value!,
          path: currentPath.value,
          filename: file.name,
          content_base64: base64
        },
        (percent, speed) => {
          uploadProgress.value = percent;
          uploadSpeed.value = speed;
        }
      );

      if (error) {
        window.$message?.error(`上传失败: ${error.message}`);
      } else {
        window.$message?.success('上传成功');
        loadFiles();
      }
    } catch (e: any) {
      window.$message?.error(`上传失败: ${e.message || 'Unknown error'}`);
    } finally {
      isUploading.value = false;
      uploadProgress.value = 0;
      uploadSpeed.value = '';
    }
  };
}

onMounted(() => {
  if (props.hostname && props.username) {
    connect();
  }
});
</script>

<template>
  <div class="h-full flex flex-col bg-white dark:bg-[#1e1e1e]">
    <!-- 表头工具栏 -->
    <div class="table-header-bar flex items-center gap-1 p-1">
      <NButton size="small" :disabled="currentPath === '/'" @click="handleGoUp">
        <template #icon>
          <NIcon><ArrowUpOutline /></NIcon>
        </template>
        上级
      </NButton>
      <NButton size="small" @click="handleRefresh">
        <template #icon>
          <NIcon><RefreshOutline /></NIcon>
        </template>
        刷新
      </NButton>
      <NButton size="small" @click="showMkdirModal = true">
        <template #icon>
          <NIcon><FolderOpenOutline /></NIcon>
        </template>
        新建
      </NButton>
      <NUpload class="inline-block" :custom-request="handleUpload" :show-file-list="false">
        <NButton size="small" :loading="isUploading">
          <template #icon>
            <NIcon><CloudUploadOutline /></NIcon>
          </template>
          上传
        </NButton>
      </NUpload>
      <div v-if="isUploading" class="ml-2 flex items-center gap-2">
        <NProgress
          type="line"
          :percentage="uploadProgress"
          :height="14"
          :border-radius="4"
          :show-indicator="false"
          processing
          class="w-32"
        />
        <span class="whitespace-nowrap text-xs text-gray-500">{{ uploadProgress }}% ({{ uploadSpeed }})</span>
      </div>

      <NBreadcrumb separator=">">
        <NBreadcrumbItem v-for="(item, index) in pathItems" :key="index" @click="handlePathClick(index)">
          <span class="cursor-pointer hover:text-primary hover:underline">{{ item }}</span>
        </NBreadcrumbItem>
      </NBreadcrumb>

      <NButton v-if="!sessionId" type="primary" size="small" class="hidden" @click="showConnectModal = true">
        连接
      </NButton>
    </div>

    <!-- File List -->
    <div class="flex-1 overflow-hidden">
      <NDataTable
        size="small"
        :columns="columns"
        :data="files"
        :loading="loading"
        :row-key="row => row.name"
        flex-height
        class="h-full"
        :bordered="false"
        striped
        :row-props="rowProps"
      />
    </div>

    <!-- Modals -->
    <!-- Connect Modal -->
    <NModal v-model:show="showConnectModal" title="连接服务器" preset="card" class="w-96">
      <NForm :model="connectForm" label-placement="left" label-width="80">
        <NFormItem label="主机">
          <NInput v-model:value="connectForm.hostname" placeholder="Hostname" />
        </NFormItem>
        <NFormItem label="端口">
          <NInputNumber v-model:value="connectForm.port" :show-button="false" class="w-full" />
        </NFormItem>
        <NFormItem label="用户名">
          <NInput v-model:value="connectForm.username" placeholder="Username" />
        </NFormItem>
        <NFormItem label="密码">
          <NInput
            v-model:value="connectForm.password"
            type="password"
            show-password-on="click"
            placeholder="Password"
          />
        </NFormItem>
        <div class="flex justify-end">
          <NButton type="primary" :loading="loading" @click="connect">连接</NButton>
        </div>
      </NForm>
    </NModal>

    <!-- Mkdir Modal -->
    <NModal v-model:show="showMkdirModal" title="新建文件夹" preset="card" class="w-96">
      <NForm :model="mkdirForm">
        <NFormItem label="文件夹名称">
          <NInput v-model:value="mkdirForm.name" placeholder="Folder Name" @keydown.enter="handleMkdir" />
        </NFormItem>
        <div class="flex justify-end">
          <NButton type="primary" @click="handleMkdir">确定</NButton>
        </div>
      </NForm>
    </NModal>

    <!-- Chmod Modal -->
    <NModal v-model:show="showChmodModal" title="修改权限" preset="card" class="w-96">
      <NForm :model="chmodForm">
        <NFormItem label="权限 (如 755)">
          <NInputNumber
            v-model:value="chmodForm.mode"
            :min="0"
            :max="777"
            class="w-full"
            @keydown.enter="handleChmod"
          />
        </NFormItem>
        <div class="flex justify-end">
          <NButton type="primary" @click="handleChmod">确定</NButton>
        </div>
      </NForm>
    </NModal>

    <!-- Text Editor Modal -->
    <TextEditor
      v-model:show="showEditor"
      :filename="editorFilename"
      :content="editorContent"
      :loading="editorLoading"
      @save="handleEditorSave"
    />
  </div>
</template>

<style scoped>
:deep(.n-data-table .n-data-table-td) {
  padding: 4px;
}
/* 隐藏原始表头，将自定义工具栏作为表头显示 */
:deep(.n-data-table .n-data-table-thead) {
  display: none;
}
/* 让自定义表头与表格对齐风格一致 */
.table-header-bar {
  border-bottom: 1px solid var(--n-border-color);
  background-color: var(--n-th-color);
}
:deep(.n-breadcrumb .n-breadcrumb-item .n-breadcrumb-item__separator) {
  margin: 0 1px !important;
}
</style>

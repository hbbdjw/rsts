<script setup lang="ts">
import { h, nextTick, onMounted, ref } from 'vue';
import {
  NButton,
  NDropdown,
  NForm,
  NFormItem,
  NIcon,
  NInput,
  NInputNumber,
  NSelect,
  NSpace,
  NSpin,
  NTree,
  useDialog,
  useMessage
} from 'naive-ui';
import type { DropdownOption, FormInst, TreeOption } from 'naive-ui';
import { Folder } from '@vicons/fa';
import {
  AddOutline,
  CodeSlashOutline,
  DesktopOutline,
  EyeOutline,
  FolderOutline,
  ListOutline,
  RefreshOutline,
  ServerOutline
} from '@vicons/ionicons5';
import { getServiceBaseURL } from '@/utils/service';
import XModal from '@/components/xmodal/index.vue';

defineOptions({
  name: 'SqlStudioMenuPanel'
});

interface ConnectionForm {
  name: string;
  dbType: string;
  host: string;
  port: number;
  username: string;
  password?: string;
  database: string;
}

const dbTypeOptions = [
  { label: 'PostgreSQL', value: 'postgresql' },
  { label: 'MySQL', value: 'mysql' },
  { label: 'SQLite3', value: 'sqlite3' },
  { label: 'DuckDB', value: 'duckdb' }
];

interface SqliteDatabaseInfo {
  name: string;
  path: string;
}

interface SqliteTableInfo {
  name: string;
  columns: Array<{
    name: string;
    data_type: string;
    not_null: boolean;
    primary_key: boolean;
  }>;
}

interface SavedConnection {
  id: number;
  name: string;
  db_type: string;
  host: string;
  port: number;
  username: string;
  database: string;
}

type NodeType = 'group' | 'connection' | 'database' | 'schema' | 'category' | 'object';
type ObjectCategory = 'tables' | 'views' | 'functions';

type SqlTreeNode = TreeOption & {
  key: string;
  label: string;
  type: NodeType;
  isLeaf?: boolean;
  children?: SqlTreeNode[];
  meta?: {
    dbName?: string;
    schemaName?: string;
    category?: ObjectCategory;
    objectType?: 'table' | 'view' | 'function';
    // For saved connections
    connectionId?: number;
    dbType?: string;
    isSqlite?: boolean;
    savedConnection?: SavedConnection;
  };
};

const message = useMessage();
const dialog = useDialog();

// Context Menu State
const showDropdown = ref(false);
const dropdownX = ref(0);
const dropdownY = ref(0);
const currentConnectionNode = ref<SqlTreeNode | null>(null);

const dropdownOptions: DropdownOption[] = [
  {
    label: '修改连接',
    key: 'edit'
  },
  {
    label: '删除连接',
    key: 'delete'
  }
];

// Edit Mode
const isEditMode = ref(false);
const editingConnectionId = ref<number | null>(null);

const loading = ref(false);
const treeData = ref<SqlTreeNode[]>([]);
const expandedKeys = ref<string[]>([]);
const selectedKeys = ref<string[]>([]);

// Modal state
const showModal = ref<boolean>(false);
const testConnLoading = ref<boolean>(false);
const saveConnLoading = ref<boolean>(false);
const formRef = ref<FormInst | null>(null);
const formModel = ref<ConnectionForm>({
  name: '',
  dbType: 'postgresql',
  host: 'localhost',
  port: 5432,
  username: 'postgres',
  password: '',
  database: 'postgres'
});

const isHttpProxy = import.meta.env.DEV && import.meta.env.VITE_HTTP_PROXY === 'Y';
const { baseURL } = getServiceBaseURL(import.meta.env, isHttpProxy);

function createNode(partial: Omit<SqlTreeNode, 'prefix'> & { icon?: any }): SqlTreeNode {
  const { icon, ...rest } = partial;
  return {
    ...rest,
    prefix: icon ? () => h(NIcon, null, { default: () => h(icon) }) : undefined
  } as SqlTreeNode;
}

async function fetchJson<T>(url: string, init?: RequestInit): Promise<T> {
  const fullUrl = url.startsWith('http') ? url : `${baseURL}${url}`;
  const res = await fetch(fullUrl, init);
  if (!res.ok) {
    const text = await res.text().catch(() => '');
    throw new Error(text || `请求失败: ${res.status}`);
  }
  const text = await res.text();
  try {
    return JSON.parse(text) as T;
  } catch (e: any) {
    throw new Error(`解析响应失败: ${e.message}. 响应内容: ${text.slice(0, 100)}...`, { cause: e });
  }
}

async function fetchSqliteDatabases(): Promise<SqliteDatabaseInfo[]> {
  return fetchJson<SqliteDatabaseInfo[]>('/api/sqlite/databases');
}

async function fetchSqliteTables(dbName: string): Promise<SqliteTableInfo[]> {
  const qs = new URLSearchParams({ db_name: dbName }).toString();
  return fetchJson<SqliteTableInfo[]>(`/api/sqlite/tables?${qs}`);
}

async function fetchSqliteQuery<T extends Record<string, any>>(
  dbName: string,
  sql: string,
  params?: any[]
): Promise<T[]> {
  const resp = await fetchJson<{ status: string; data?: T[]; changed?: number }>(`/api/sqlite/query`, {
    method: 'post',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({
      db_name: dbName,
      sql,
      params: params ?? []
    })
  });
  return resp.data ?? [];
}

async function buildRootTree() {
  loading.value = true;
  try {
    const [sqliteDbs, connections] = await Promise.all([
      fetchSqliteDatabases(),
      fetchJson<{ code: number; data: SavedConnection[] }>('/api/sqlstudio/connection/list').then(res => res.data || [])
    ]);

    const children: SqlTreeNode[] = [];

    // Add Saved Connections
    if (connections.length > 0) {
      const savedGroup = createNode({
        key: 'group-saved',
        label: '已保存连接',
        type: 'group',
        icon: Folder,
        children: connections.map(conn =>
          createNode({
            key: `saved-conn-${conn.id}`,
            label: conn.name,
            type: 'connection',
            icon: DesktopOutline,
            meta: {
              connectionId: conn.id,
              dbType: conn.db_type,
              dbName: conn.database, // Default database
              savedConnection: conn
            }
          })
        )
      });
      children.push(savedGroup);
    }

    // Add SQLite Databases (existing logic)
    if (sqliteDbs.length > 0) {
      const sqliteGroup = createNode({
        key: 'group-sqlite',
        label: '本地 SQLite',
        type: 'group',
        icon: Folder,
        children: sqliteDbs.map(db =>
          createNode({
            key: `conn-${encodeURIComponent(db.name)}`,
            label: db.name,
            type: 'connection',
            icon: DesktopOutline,
            meta: { dbName: db.name, isSqlite: true }
          })
        )
      });
      children.push(sqliteGroup);
    }

    // Fallback if empty or just merge
    if (children.length === 0) {
      const groupNode = createNode({
        key: 'group-default',
        label: '默认分组',
        type: 'group',
        icon: Folder,
        children: []
      });
      treeData.value = [groupNode];
    } else {
      treeData.value = children;
    }
  } catch (e: any) {
    message.error(e?.message || '加载数据库列表失败');
    treeData.value = [];
  } finally {
    loading.value = false;
  }
}

async function loadSavedConnectionNode(node: SqlTreeNode) {
  const { connectionId } = node.meta || {};
  if (!connectionId) return;

  try {
    // List databases
    const res = await fetchJson<{ code: number; data: { name: string; object_type: string }[] }>(
      '/api/sqlstudio/connection/metadata',
      {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          connection_id: connectionId,
          action: 'databases'
        })
      }
    );

    if (res.code === 0 && res.data) {
      node.children = res.data.map(db =>
        createNode({
          key: `saved-db-${connectionId}-${encodeURIComponent(db.name)}`,
          label: db.name,
          type: 'database',
          icon: ServerOutline,
          meta: { connectionId, dbName: db.name }
        })
      );
    } else {
      message.error('加载数据库失败');
    }
  } catch (e: any) {
    message.error(e.message || '加载数据库失败');
  }
}

async function loadSavedDatabaseNode(node: SqlTreeNode) {
  const { connectionId, dbName } = node.meta || {};
  if (!connectionId || !dbName) return;

  try {
    // List schemas
    const res = await fetchJson<{ code: number; data: { name: string; object_type: string }[] }>(
      '/api/sqlstudio/connection/metadata',
      {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          connection_id: connectionId,
          action: 'schemas',
          database: dbName
        })
      }
    );

    if (res.code === 0 && res.data) {
      node.children = res.data.map(schema =>
        createNode({
          key: `saved-schema-${connectionId}-${encodeURIComponent(dbName)}-${encodeURIComponent(schema.name)}`,
          label: schema.name,
          type: 'schema',
          icon: FolderOutline,
          meta: { connectionId, dbName, schemaName: schema.name }
        })
      );
    } else {
      message.error('加载Schema失败');
    }
  } catch (e: any) {
    message.error(e.message || '加载Schema失败');
  }
}

async function loadSavedSchemaNode(node: SqlTreeNode) {
  const { connectionId, dbName, schemaName } = node.meta || {};
  if (!connectionId || !dbName || !schemaName) return;

  const baseKey = `saved-schema-${connectionId}-${encodeURIComponent(dbName)}-${encodeURIComponent(schemaName)}`;
  node.children = [
    createNode({
      key: `${baseKey}-cat-tables`,
      label: 'tables',
      type: 'category',
      icon: ListOutline,
      meta: { connectionId, dbName, schemaName, category: 'tables' }
    }),
    createNode({
      key: `${baseKey}-cat-views`,
      label: 'views',
      type: 'category',
      icon: EyeOutline,
      meta: { connectionId, dbName, schemaName, category: 'views' }
    }),
    createNode({
      key: `${baseKey}-cat-functions`,
      label: 'functions',
      type: 'category',
      icon: CodeSlashOutline,
      meta: { connectionId, dbName, schemaName, category: 'functions' }
    })
  ];
}

async function loadSavedCategoryNode(node: SqlTreeNode) {
  const { connectionId, dbName, schemaName, category } = node.meta || {};
  if (!connectionId || !dbName || !schemaName || !category) return;

  try {
    const res = await fetchJson<{ code: number; data: { name: string; object_type: string }[] }>(
      '/api/sqlstudio/connection/metadata',
      {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          connection_id: connectionId,
          action: category,
          database: dbName,
          schema: schemaName
        })
      }
    );

    if (res.code === 0 && res.data) {
      node.children = res.data.map(obj => {
        let objectType: 'table' | 'view' | 'function' = 'function';
        if (category === 'tables') objectType = 'table';
        else if (category === 'views') objectType = 'view';

        return createNode({
          key: `saved-obj-${connectionId}-${encodeURIComponent(dbName)}-${encodeURIComponent(schemaName)}-${category}-${encodeURIComponent(obj.name)}`,
          label: obj.name,
          type: 'object',
          isLeaf: true,
          meta: {
            connectionId,
            dbName,
            schemaName,
            objectType
          }
        });
      });
    } else {
      message.error(`加载${category}失败`);
    }
  } catch (e: any) {
    message.error(e.message || `加载${category}失败`);
  }
}

function loadConnectionNode(node: SqlTreeNode) {
  // Existing SQLite logic
  const dbName = node.meta?.dbName;
  if (!dbName) return;
  node.children = [
    createNode({
      key: `db-${encodeURIComponent(dbName)}`,
      label: dbName,
      type: 'database',
      icon: ServerOutline,
      meta: { dbName, isSqlite: true }
    })
  ];
}

function loadDatabaseNode(node: SqlTreeNode) {
  const dbName = node.meta?.dbName;
  if (!dbName) return;
  node.children = [
    createNode({
      key: `schema-${encodeURIComponent(dbName)}-main`,
      label: 'main',
      type: 'schema',
      icon: FolderOutline,
      meta: { dbName, schemaName: 'main', isSqlite: true }
    })
  ];
}

function loadSchemaNode(node: SqlTreeNode) {
  const dbName = node.meta?.dbName;
  const schemaName = node.meta?.schemaName;
  if (!dbName || !schemaName) return;
  const baseKey = `schema-${encodeURIComponent(dbName)}-${encodeURIComponent(schemaName)}`;
  node.children = [
    createNode({
      key: `${baseKey}-cat-tables`,
      label: 'tables',
      type: 'category',
      icon: ListOutline,
      meta: { dbName, schemaName, category: 'tables', isSqlite: true }
    }),
    createNode({
      key: `${baseKey}-cat-views`,
      label: 'views',
      type: 'category',
      icon: EyeOutline,
      meta: { dbName, schemaName, category: 'views', isSqlite: true }
    }),
    createNode({
      key: `${baseKey}-cat-functions`,
      label: 'functions',
      type: 'category',
      icon: CodeSlashOutline,
      isLeaf: true,
      meta: { dbName, schemaName, category: 'functions', isSqlite: true }
    })
  ];
}

async function loadCategoryNode(node: SqlTreeNode) {
  const dbName = node.meta?.dbName;
  const category = node.meta?.category;
  if (!dbName || !category) return;

  if (category === 'tables') {
    const tables = await fetchSqliteTables(dbName);
    node.children = tables.map(t =>
      createNode({
        key: `table-${encodeURIComponent(dbName)}-${encodeURIComponent(t.name)}`,
        label: t.name,
        type: 'object',
        isLeaf: true,
        meta: { dbName, objectType: 'table', isSqlite: true }
      })
    );
    return;
  }

  if (category === 'views') {
    const rows = await fetchSqliteQuery<{ name: string }>(
      dbName,
      "SELECT name FROM sqlite_master WHERE type='view' AND name NOT LIKE 'sqlite_%' ORDER BY name"
    );
    node.children = rows.map(r =>
      createNode({
        key: `view-${encodeURIComponent(dbName)}-${encodeURIComponent(r.name)}`,
        label: r.name,
        type: 'object',
        isLeaf: true,
        meta: { dbName, objectType: 'view', isSqlite: true }
      })
    );
  }
}

async function handleLoad(rawNode: TreeOption) {
  const node = rawNode as SqlTreeNode;
  if (node.children && node.children.length > 0) return;

  // Saved Connections Logic
  if (node.meta?.connectionId) {
    if (node.type === 'connection') {
      await loadSavedConnectionNode(node);
    } else if (node.type === 'database') {
      await loadSavedDatabaseNode(node);
    } else if (node.type === 'schema') {
      await loadSavedSchemaNode(node);
    } else if (node.type === 'category') {
      await loadSavedCategoryNode(node);
    }
    return;
  }

  // SQLite Logic
  if (node.type === 'connection') {
    loadConnectionNode(node);
    return;
  }

  if (node.type === 'database') {
    loadDatabaseNode(node);
    return;
  }

  if (node.type === 'schema') {
    loadSchemaNode(node);
    return;
  }

  if (node.type === 'category') {
    await loadCategoryNode(node);
  }
}

function handleUpdateExpandedKeys(keys: string[]) {
  expandedKeys.value = keys;
}

function handleUpdateSelectedKeys(keys: string[]) {
  selectedKeys.value = keys;
}

async function reload() {
  expandedKeys.value = [];
  selectedKeys.value = [];
  await buildRootTree();
}

function handleAddConnection() {
  formModel.value = {
    name: '',
    dbType: 'postgresql',
    host: 'localhost',
    port: 5432,
    username: 'postgres',
    password: '',
    database: 'postgres'
  };
  isEditMode.value = false;
  editingConnectionId.value = null;
  showModal.value = true;
}

async function handleTestConnection() {
  testConnLoading.value = true;
  try {
    const res = await fetchJson<{ code: number; msg: string }>('/api/sqlstudio/connection/test', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        db_type: formModel.value.dbType,
        host: formModel.value.host,
        port: formModel.value.port,
        username: formModel.value.username,
        password: formModel.value.password,
        database: formModel.value.database
      })
    });
    if (res.code === 0) {
      message.success('连接成功');
    } else {
      message.error(`连接失败: ${res.msg}`);
    }
  } catch (e: any) {
    message.error(e.message || '连接测试失败');
  } finally {
    testConnLoading.value = false;
  }
}

async function handleSaveConnection() {
  formRef.value?.validate(async errors => {
    if (!errors) {
      saveConnLoading.value = true;
      try {
        let url = '/api/sqlstudio/connection/create';
        const body: any = {
          name: formModel.value.name,
          db_type: formModel.value.dbType,
          host: formModel.value.host,
          port: formModel.value.port,
          username: formModel.value.username,
          password: formModel.value.password || undefined,
          database: formModel.value.database
        };

        if (isEditMode.value && editingConnectionId.value) {
          url = '/api/sqlstudio/connection/update';
          body.id = editingConnectionId.value;
        }

        const res = await fetchJson<{ code: number; msg: string }>(url, {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify(body)
        });
        if (res.code === 0) {
          message.success('保存成功');
          showModal.value = false;
          await reload();
        } else {
          message.error(`保存失败: ${res.msg}`);
        }
      } catch (e: any) {
        message.error(e.message || '保存失败');
      } finally {
        saveConnLoading.value = false;
      }
    }
  });
}

function handleSelect(key: string | number) {
  showDropdown.value = false;
  if (!currentConnectionNode.value?.meta?.savedConnection) return;
  const conn = currentConnectionNode.value.meta.savedConnection;

  if (key === 'edit') {
    isEditMode.value = true;
    editingConnectionId.value = conn.id;
    formModel.value = {
      name: conn.name,
      dbType: conn.db_type,
      host: conn.host,
      port: conn.port,
      username: conn.username,
      password: '', // Don't show password
      database: conn.database
    };
    showModal.value = true;
  } else if (key === 'delete') {
    dialog.warning({
      title: '确认删除',
      content: `确定要删除连接 "${conn.name}" 吗？`,
      positiveText: '确定',
      negativeText: '取消',
      onPositiveClick: () => handleDeleteConnection(conn.id)
    });
  }
}

async function handleDeleteConnection(id: number) {
  try {
    const res = await fetchJson<{ code: number; msg: string }>('/api/sqlstudio/connection/delete', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ id })
    });
    if (res.code === 0) {
      message.success('删除成功');
      await reload();
    } else {
      message.error(`删除失败: ${res.msg}`);
    }
  } catch (e: any) {
    message.error(e.message || '删除失败');
  }
}

function handleContextMenu(node: SqlTreeNode, event: MouseEvent) {
  if (node.type === 'connection' && node.meta?.savedConnection) {
    event.preventDefault();
    showDropdown.value = false;
    nextTick(() => {
      showDropdown.value = true;
      dropdownX.value = event.clientX;
      dropdownY.value = event.clientY;
      currentConnectionNode.value = node;
    });
  }
}

const nodeProps = ({ option }: { option: TreeOption }) => {
  return {
    onContextmenu(e: MouseEvent) {
      handleContextMenu(option as SqlTreeNode, e);
    },
    async onDblclick(e: MouseEvent) {
      // 阻止默认行为
      e.preventDefault();
      e.stopPropagation();

      // 如果是叶子节点，不处理展开
      if (option.isLeaf) return;

      const key = option.key as string;
      const index = expandedKeys.value.indexOf(key);

      // 切换展开状态
      if (index > -1) {
        expandedKeys.value = expandedKeys.value.filter(k => k !== key);
        return;
      }

      // 使用新数组赋值以确保响应式更新
      expandedKeys.value = [...expandedKeys.value, key];
      await handleLoad(option);
    }
  };
};

onMounted(buildRootTree);
</script>

<template>
  <div class="menu-panel bs-shadow-md h-full w-full flex flex-col overflow-hidden">
    <div class="flex shrink-0 items-center gap-1 p-1">
      <NButton size="small" @click="handleAddConnection">
        <template #icon>
          <NIcon>
            <AddOutline />
          </NIcon>
        </template>
      </NButton>
      <NButton size="small" @click="reload">
        <template #icon>
          <NIcon>
            <RefreshOutline />
          </NIcon>
        </template>
      </NButton>
    </div>
    <div class="flex-1 overflow-hidden">
      <NSpin :show="loading" class="h-full">
        <NTree
          :data="treeData"
          block-line
          :expanded-keys="expandedKeys"
          :selected-keys="selectedKeys"
          :node-props="nodeProps"
          :on-load="handleLoad"
          @update:expanded-keys="handleUpdateExpandedKeys"
          @update:selected-keys="handleUpdateSelectedKeys"
        />
        <NDropdown
          placement="bottom-start"
          trigger="manual"
          :x="dropdownX"
          :y="dropdownY"
          :options="dropdownOptions"
          :show="showDropdown"
          :on-clickoutside="() => (showDropdown = false)"
          @select="handleSelect"
        />
      </NSpin>
    </div>

    <XModal
      v-model:show="showModal"
      preset="card"
      :title="isEditMode ? '编辑连接' : '新建连接'"
      :width="800"
      :height="550"
      :show-mask="true"
    >
      <NForm
        ref="formRef"
        :model="formModel"
        label-placement="left"
        label-width="100"
        require-mark-placement="right-hanging"
      >
        <NFormItem
          label="连接名称"
          path="name"
          rule-path="name"
          :rule="{ required: true, message: '请输入连接名称', trigger: 'blur' }"
        >
          <NInput v-model:value="formModel.name" placeholder="请输入连接名称" />
        </NFormItem>
        <NFormItem label="数据库类型" path="dbType">
          <NSelect v-model:value="formModel.dbType" :options="dbTypeOptions" />
        </NFormItem>
        <NFormItem label="Host" path="host" :rule="{ required: true, message: '请输入Host', trigger: 'blur' }">
          <NInput v-model:value="formModel.host" placeholder="localhost" />
        </NFormItem>
        <NFormItem
          label="Port"
          path="port"
          :rule="{ required: true, type: 'number', message: '请输入端口', trigger: 'blur' }"
        >
          <NInputNumber v-model:value="formModel.port" placeholder="5432" />
        </NFormItem>
        <NFormItem
          label="Database"
          path="database"
          :rule="{ required: true, message: '请输入数据库名', trigger: 'blur' }"
        >
          <NInput v-model:value="formModel.database" placeholder="postgres" />
        </NFormItem>
        <NFormItem
          label="Username"
          path="username"
          :rule="{ required: true, message: '请输入用户名', trigger: 'blur' }"
        >
          <NInput v-model:value="formModel.username" placeholder="postgres" />
        </NFormItem>
        <NFormItem label="Password" path="password">
          <NInput
            v-model:value="formModel.password"
            type="password"
            show-password-on="click"
            placeholder="请输入密码"
          />
        </NFormItem>
      </NForm>
      <template #footer>
        <NSpace justify="end">
          <NButton :loading="testConnLoading" @click="handleTestConnection">测试连接</NButton>
          <NButton type="primary" :loading="saveConnLoading" @click="handleSaveConnection">保存</NButton>
        </NSpace>
      </template>
    </XModal>
  </div>
</template>

<style scoped>
:deep(.n-tree .n-tree-node-wrapper) {
  padding: 0;
}
</style>

<!-- ssh左侧面板，树形分组 -->
<script setup lang="ts">
import { computed, h, onMounted, reactive, ref } from 'vue';
import {
  NButton,
  NForm,
  NFormItem,
  NIcon,
  NInput,
  NInputNumber,
  NModal,
  NSelect,
  NSpace,
  NTree,
  useMessage
} from 'naive-ui';
import { Folder } from '@vicons/fa';
import { AddOutline, DesktopOutline } from '@vicons/ionicons5';
import { request } from '@/service/request';

const emit = defineEmits<{
  (e: 'connect', payload: any): void;
}>();
const message = useMessage();

interface GroupItem {
  id: number;
  name: string;
  is_default: number;
}

interface ServerItem {
  id: number;
  alias: string;
  hostname: string;
  port: number;
  username: string;
  password?: string | null;
  group_id: number;
  remark?: string | null;
}

const groups = ref<GroupItem[]>([]);
const servers = ref<ServerItem[]>([]);
const selectedKeys = ref<string[]>([]);

const groupKey = (id: number) => `group-${id}`;
const serverKey = (id: number) => `server-${id}`;

const defaultGroupId = computed(() => groups.value.find(g => g.is_default === 1)?.id ?? groups.value[0]?.id ?? null);

const treeData = computed(() => {
  return groups.value.map(group => {
    const children = servers.value
      .filter(s => (s.group_id ?? defaultGroupId.value) === group.id)
      .map(s => ({
        key: serverKey(s.id),
        label: s.alias && s.alias.trim().length > 0 ? s.alias : s.hostname,
        type: 'server',
        isLeaf: true,
        server: s,
        prefix: () => h(NIcon, null, { default: () => h(DesktopOutline) })
      }));
    return {
      key: groupKey(group.id),
      label: group.name,
      type: 'group',
      children,
      prefix: () => h(NIcon, null, { default: () => h(Folder) })
    };
  });
});

const groupOptions = computed(() => groups.value.map(g => ({ label: g.name, value: g.id })));

const showConnectModal = ref(false);
const showAddGroupModal = ref(false);
const showRenameGroupModal = ref(false);
const showDeleteGroupModal = ref(false);

const connectForm = reactive({
  hostname: '',
  port: 22,
  username: '',
  password: '',
  group_id: null as number | null,
  alias: '',
  remark: ''
});

const groupForm = reactive({ name: '' });
const renameGroupForm = reactive({ name: '' });

const fetchGroups = async () => {
  const { data, error } = await request<GroupItem[]>({
    url: '/api/ssh/groups',
    method: 'get'
  });
  if (error) {
    message.error(error.message || '加载分组失败');
    return;
  }
  groups.value = data || [];
};

const fetchServers = async () => {
  const { data, error } = await request<ServerItem[]>({
    url: '/api/ssh/servers',
    method: 'get'
  });
  if (error) {
    message.error(error.message || '加载服务器失败');
    return;
  }
  servers.value = data || [];
};

const loadData = async () => {
  await Promise.all([fetchGroups(), fetchServers()]);
};

onMounted(loadData);

const getSelectedGroup = () => {
  const key = selectedKeys.value[0];
  if (!key || !key.startsWith('group-')) return null;
  const id = Number(key.replace('group-', ''));
  return groups.value.find(g => g.id === id) || null;
};

const handleUpdateSelectedKeys = (keys: string[], options: any[]) => {
  selectedKeys.value = keys;
  const node = options?.[0];
  if (node?.type === 'server' && node.server) {
    emit('connect', {
      hostname: node.server.hostname,
      port: node.server.port,
      username: node.server.username,
      password: node.server.password || ''
    });
  }
};

const handleAddConnection = () => {
  connectForm.hostname = '';
  connectForm.port = 22;
  connectForm.username = '';
  connectForm.password = '';
  connectForm.alias = '';
  connectForm.remark = '';
  connectForm.group_id = getSelectedGroup()?.id ?? defaultGroupId.value;
  showConnectModal.value = true;
};

const handleConnect = () => {
  if (!connectForm.hostname || !connectForm.username) {
    message.error('请输入主机名和用户名');
    return;
  }
  emit('connect', {
    hostname: connectForm.hostname,
    port: connectForm.port,
    username: connectForm.username,
    password: connectForm.password
  });
  showConnectModal.value = false;
};

const handleSaveServer = async () => {
  if (!connectForm.hostname || !connectForm.username) {
    message.error('请输入主机名和用户名');
    return;
  }
  const payload = {
    hostname: connectForm.hostname,
    port: connectForm.port,
    username: connectForm.username,
    password: connectForm.password,
    alias: connectForm.alias || '',
    remark: connectForm.remark || '',
    group_id: connectForm.group_id ?? defaultGroupId.value
  };
  const { error } = await request<ServerItem>({
    url: '/api/ssh/servers',
    method: 'post',
    data: payload
  });
  if (error) {
    message.error(error.message || '保存失败');
    return;
  }
  message.success('保存成功');
  showConnectModal.value = false;
  await loadData();
};

const handleOpenAddGroup = () => {
  groupForm.name = '';
  showAddGroupModal.value = true;
};

const handleCreateGroup = async () => {
  if (!groupForm.name) {
    message.error('请输入分组名称');
    return;
  }
  const { error } = await request<GroupItem>({
    url: '/api/ssh/groups',
    method: 'post',
    data: { name: groupForm.name }
  });
  if (error) {
    message.error(error.message || '新增失败');
    return;
  }
  showAddGroupModal.value = false;
  await loadData();
};

const handleOpenRenameGroup = () => {
  const group = getSelectedGroup();
  if (!group) {
    message.warning('请选择分组');
    return;
  }
  renameGroupForm.name = group.name;
  showRenameGroupModal.value = true;
};

const handleRenameGroup = async () => {
  const group = getSelectedGroup();
  if (!group) {
    message.warning('请选择分组');
    return;
  }
  if (!renameGroupForm.name) {
    message.error('请输入分组名称');
    return;
  }
  const { error } = await request<GroupItem>({
    url: `/api/ssh/groups/${group.id}`,
    method: 'put',
    data: { name: renameGroupForm.name }
  });
  if (error) {
    message.error(error.message || '重命名失败');
    return;
  }
  showRenameGroupModal.value = false;
  await loadData();
};

const handleOpenDeleteGroup = () => {
  const group = getSelectedGroup();
  if (!group) {
    message.warning('请选择分组');
    return;
  }
  showDeleteGroupModal.value = true;
};

const handleDeleteGroup = async () => {
  const group = getSelectedGroup();
  if (!group) {
    message.warning('请选择分组');
    return;
  }
  const { error } = await request<{ id: number }>({
    url: `/api/ssh/groups/${group.id}`,
    method: 'delete'
  });
  if (error) {
    message.error(error.message || '删除失败');
    return;
  }
  showDeleteGroupModal.value = false;
  await loadData();
};
</script>

<template>
  <div class="ssh-left h-full flex flex-col">
    <div class="flex items-center border-b border-gray-200 p-2 dark:border-gray-700">
      <NSpace size="small">
        <NButton size="tiny" ghost @click="handleAddConnection">
          <template #icon>
            <NIcon><AddOutline /></NIcon>
          </template>
          新增
        </NButton>
        <NButton size="tiny" ghost @click="handleOpenAddGroup">新增分组</NButton>
        <NButton size="tiny" ghost @click="handleOpenRenameGroup">重命名</NButton>
        <NButton size="tiny" ghost @click="handleOpenDeleteGroup">删除</NButton>
      </NSpace>
    </div>
    <div class="flex-1 overflow-auto p-2">
      <NTree
        block-line
        :data="treeData"
        :selected-keys="selectedKeys"
        @update:selected-keys="handleUpdateSelectedKeys"
      />
    </div>

    <NModal v-model:show="showAddGroupModal" title="新增分组" preset="card" class="w-96">
      <NForm :model="groupForm" label-placement="left" label-width="80">
        <NFormItem label="分组名称">
          <NInput v-model:value="groupForm.name" placeholder="请输入分组名称" />
        </NFormItem>
        <div class="flex justify-end">
          <NButton type="primary" @click="handleCreateGroup">保存</NButton>
        </div>
      </NForm>
    </NModal>

    <NModal v-model:show="showRenameGroupModal" title="重命名分组" preset="card" class="w-96">
      <NForm :model="renameGroupForm" label-placement="left" label-width="80">
        <NFormItem label="分组名称">
          <NInput v-model:value="renameGroupForm.name" placeholder="请输入分组名称" />
        </NFormItem>
        <div class="flex justify-end">
          <NButton type="primary" @click="handleRenameGroup">保存</NButton>
        </div>
      </NForm>
    </NModal>

    <NModal v-model:show="showDeleteGroupModal" title="删除分组" preset="card" class="w-96">
      <div class="text-sm text-gray-500">确认删除该分组？该分组下的服务器将移动到默认分组。</div>
      <div class="mt-4 flex justify-end">
        <NButton type="primary" @click="handleDeleteGroup">确认</NButton>
      </div>
    </NModal>

    <NModal v-model:show="showConnectModal" title="新增连接服务器" preset="card" class="w-96">
      <NForm :model="connectForm" label-placement="left" label-width="80">
        <NFormItem label="分组">
          <NSelect v-model:value="connectForm.group_id" :options="groupOptions" clearable />
        </NFormItem>
        <NFormItem label="服务器别名">
          <NInput v-model:value="connectForm.alias" placeholder="可选" />
        </NFormItem>
        <NFormItem label="备注">
          <NInput v-model:value="connectForm.remark" type="textarea" />
        </NFormItem>
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
        <div class="flex justify-end gap-2">
          <NButton @click="handleSaveServer">保存</NButton>
          <NButton type="primary" @click="handleConnect">连接</NButton>
        </div>
      </NForm>
    </NModal>
  </div>
</template>

<style lang="css">
:deep(.n-tree .n-tree-node-wrapper) {
  padding: 0;
}
</style>

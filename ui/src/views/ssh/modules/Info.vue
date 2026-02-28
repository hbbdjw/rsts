<script setup lang="ts">
export interface ConnectionInfo {
  id: string;
  username: string;
  hostname: string;
  port: number | string;
}

const { connections, activeId } = defineProps<{
  connections: ConnectionInfo[];
  activeId: string | null;
}>();

const emit = defineEmits(['update:activeId', 'close']);

const handleUpdateValue = (value: string) => {
  emit('update:activeId', value);
};

const handleClose = (value: string) => {
  emit('close', value);
};
</script>

<template>
  <div class="h-full w-full flex items-center overflow-hidden">
    <NTabs
      v-if="connections.length > 0"
      type="card"
      closable
      :value="activeId ?? undefined"
      class="h-full w-full"
      tab-style="min-width: 100px;"
      @update:value="handleUpdateValue"
      @close="handleClose"
    >
      <NTabPane v-for="conn in connections" :key="conn.id" :name="conn.id" :tab="conn.hostname" />
    </NTabs>
    <div v-else class="px-4 text-sm text-gray-400">No active connection</div>
  </div>
</template>

<style scoped>
:deep(.n-tabs-nav) {
  height: 100%;
}
:deep(.n-tabs-wrapper) {
  height: 100%;
}
:deep(.n-tabs-tab-wrapper) {
  height: 100%;
  align-items: flex-end;
}
:deep(.n-tab-pane) {
  display: none;
}
:deep(.n-tabs-content) {
  display: none;
}
:deep(.n-tabs-tab) {
  border: 0px !important;
  border-radius: 0 !important;
  padding-top: 6px !important;
  padding-bottom: 3px !important;
  padding-left: 5px !important;
  padding-right: 5px !important;
}
:deep(.n-tabs-tab--active) {
  background-color: #1e1e1e !important;
  color: #00cd00 !important;
  font-weight: 800 !important;
  border-top: 2px #00cd00 !important;
}
:deep(.n-tabs-pad) {
  border-bottom: 0px !important;
}
:deep(.n-base-close) {
  color: wheat;
}
</style>

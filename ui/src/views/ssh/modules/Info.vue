<script setup lang="ts">
import { onUnmounted, ref } from 'vue';
defineOptions({
  name: 'SshInfo'
});

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

const emit = defineEmits<{
  (e: 'update:activeId', value: string): void;
  (e: 'close', value: string): void;
  (e: 'detach', payload: { id: string; x: number; y: number }): void;
}>();

const handleUpdateValue = (value: string) => {
  emit('update:activeId', value);
};

const handleClose = (value: string) => {
  emit('close', value);
};

const rootRef = ref<HTMLElement | null>(null);
const dragState = ref<{
  dragging: boolean;
  moved: boolean;
  startX: number;
  startY: number;
  id: string | null;
}>({
  dragging: false,
  moved: false,
  startX: 0,
  startY: 0,
  id: null
});

const handleMouseMove = (e: MouseEvent) => {
  if (!dragState.value.dragging) return;
  const dx = Math.abs(e.clientX - dragState.value.startX);
  const dy = Math.abs(e.clientY - dragState.value.startY);
  if (dx > 6 || dy > 6) {
    dragState.value.moved = true;
  }
};

const handleMouseUp = (e: MouseEvent) => {
  if (!dragState.value.dragging) return;
  const { id, moved } = dragState.value;
  dragState.value = { dragging: false, moved: false, startX: 0, startY: 0, id: null };
  window.removeEventListener('mousemove', handleMouseMove);
  window.removeEventListener('mouseup', handleMouseUp);
  if (!id || !moved) return;
  const nav = rootRef.value?.querySelector('.n-tabs-nav') as HTMLElement | null;
  const rect = nav?.getBoundingClientRect();
  if (!rect) return;
  const margin = 12;
  const outX = e.clientX < rect.left - margin || e.clientX > rect.right + margin;
  const outY = e.clientY < rect.top - margin || e.clientY > rect.bottom + margin;
  if (outX || outY) {
    emit('detach', { id, x: e.clientX, y: e.clientY });
  }
};

const handleTabMouseDown = (e: MouseEvent, id: string) => {
  if (e.button !== 0) return;
  e.preventDefault();
  dragState.value = {
    dragging: true,
    moved: false,
    startX: e.clientX,
    startY: e.clientY,
    id
  };
  window.addEventListener('mousemove', handleMouseMove);
  window.addEventListener('mouseup', handleMouseUp);
};

onUnmounted(() => {
  window.removeEventListener('mousemove', handleMouseMove);
  window.removeEventListener('mouseup', handleMouseUp);
});
</script>

<template>
  <div ref="rootRef" class="h-full w-full flex items-center overflow-hidden">
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
      <NTabPane v-for="conn in connections" :key="conn.id" :name="conn.id">
        <template #tab>
          <div class="ssh-tab-label" @mousedown="e => handleTabMouseDown(e, conn.id)">
            {{ conn.hostname }}
          </div>
        </template>
      </NTabPane>
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

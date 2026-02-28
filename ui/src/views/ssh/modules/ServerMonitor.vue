<script setup lang="ts">
import { onMounted, onUnmounted, reactive, ref, watch } from 'vue';
import { NCard, NColorPicker, NForm, NFormItem, NIcon, NInputNumber, NModal, NProgress } from 'naive-ui';
import { CloseOutline, PulseOutline, SettingsOutline } from '@vicons/ionicons5';

const showPanel = ref(false);
const showSettings = ref(false);
const timer = ref<NodeJS.Timeout | null>(null);

const emit = defineEmits<{
  (e: 'update-settings', settings: any): void;
}>();

const props = defineProps<{
  initialSettings?: {
    fontSize: number;
    background: string;
    foreground: string;
  };
}>();

const settings = reactive({
  fontSize: props.initialSettings?.fontSize ?? 14,
  background: props.initialSettings?.background ?? '#1e1e1e',
  foreground: props.initialSettings?.foreground ?? '#d4d4d4'
});

watch(
  settings,
  newVal => {
    emit('update-settings', {
      fontSize: newVal.fontSize,
      theme: {
        background: newVal.background,
        foreground: newVal.foreground
      }
    });
  },
  { deep: true }
);

interface SystemStats {
  cpu: number;
  memory: {
    used: number;
    total: number;
    usage: number;
  };
  swap: {
    used: number;
    total: number;
    usage: number;
  };
}

const stats = ref<SystemStats>({
  cpu: 0,
  memory: { used: 0, total: 0, usage: 0 },
  swap: { used: 0, total: 0, usage: 0 }
});

const fetchStats = async () => {
  stats.value = {
    cpu: Math.floor(Math.random() * 100),
    memory: {
      used: Math.floor(Math.random() * 8192),
      total: 16384,
      usage: Math.floor(Math.random() * 100)
    },
    swap: {
      used: Math.floor(Math.random() * 2048),
      total: 4096,
      usage: Math.floor(Math.random() * 100)
    }
  };
};

const startPolling = () => {
  fetchStats();
  if (timer.value) clearInterval(timer.value);
  timer.value = setInterval(fetchStats, 2000);
};

const stopPolling = () => {
  if (timer.value) {
    clearInterval(timer.value);
    timer.value = null;
  }
};

const togglePanel = () => {
  showPanel.value = !showPanel.value;
  if (showPanel.value) {
    startPolling();
  } else {
    stopPolling();
  }
};

onMounted(() => {
  // Optional: Start polling if panel is open by default, currently it's closed
});

onUnmounted(() => {
  stopPolling();
});
</script>

<template>
  <div class="h-full w-full">
    <!-- Stats Panel: Top Right -->
    <div v-if="showPanel" class="pointer-events-auto absolute right-0 top-0 z-10">
      <NCard
        size="small"
        class="w-64 !rounded-none !border-none !bg-opacity-80 !backdrop-blur-sm"
        :content-style="{ padding: '8px' }"
      >
        <div class="absolute right-1 top-1 z-10 cursor-pointer" @click="togglePanel">
          <NIcon size="16" class="text-gray-400 hover:text-white"><CloseOutline /></NIcon>
        </div>

        <div class="mt-2 space-y-2">
          <!-- CPU -->
          <div class="flex items-center gap-2">
            <div class="w-10 text-xs text-gray-300">CPU</div>
            <div class="flex-1">
              <NProgress
                type="line"
                :percentage="stats.cpu"
                :color="stats.cpu > 80 ? '#d03050' : '#18a058'"
                :height="10"
                indicator-placement="inside"
                processing
                :show-indicator="false"
              />
            </div>
          </div>

          <!-- Memory -->
          <div class="flex items-center gap-2">
            <div class="w-10 text-xs text-gray-300">MEM</div>
            <div class="flex-1">
              <NProgress
                type="line"
                :percentage="stats.memory.usage"
                :show-indicator="false"
                :color="stats.memory.usage > 80 ? '#d03050' : '#2080f0'"
                :height="10"
              />
            </div>
          </div>

          <!-- Swap -->
          <div class="flex items-center gap-2">
            <div class="w-10 text-xs text-gray-300">SWAP</div>
            <div class="flex-1">
              <NProgress
                type="line"
                :percentage="stats.swap.usage"
                :show-indicator="false"
                :color="stats.swap.usage > 80 ? '#d03050' : '#f0a020'"
                :height="10"
              />
            </div>
          </div>
        </div>
      </NCard>
    </div>

    <!-- Buttons: Bottom Right -->
    <div class="pointer-events-auto absolute bottom-2 right-2 z-10 flex flex-col items-end gap-1">
      <!-- Toggle Button -->
      <div
        class="cursor-pointer bg-black bg-opacity-50 p-1 text-white transition-colors hover:bg-opacity-70"
        @click="togglePanel"
      >
        <NIcon size="20"><PulseOutline /></NIcon>
      </div>

      <!-- Settings Button -->
      <div
        class="cursor-pointer bg-black bg-opacity-50 p-1 text-white transition-colors hover:bg-opacity-70"
        @click="showSettings = true"
      >
        <NIcon size="20"><SettingsOutline /></NIcon>
      </div>
    </div>

    <!-- Settings Modal -->
    <NModal
      v-model:show="showSettings"
      preset="card"
      title="Terminal Settings"
      class="w-80"
      :segmented="{
        content: true,
        footer: 'soft'
      }"
    >
      <NForm size="small" label-placement="left" label-width="auto">
        <NFormItem label="Font Size">
          <NInputNumber v-model:value="settings.fontSize" :min="10" :max="32" />
        </NFormItem>
        <NFormItem label="Background">
          <NColorPicker v-model:value="settings.background" :show-alpha="false" />
        </NFormItem>
        <NFormItem label="Foreground">
          <NColorPicker v-model:value="settings.foreground" :show-alpha="false" />
        </NFormItem>
      </NForm>
    </NModal>
  </div>
</template>

<style scoped>
:deep(.n-card) {
  background-color: rgba(0, 0, 0, 0.6) !important;
  color: white;
}
:deep(.n-progress-graph-line-rail) {
  background-color: rgba(255, 255, 255, 0.2);
}
</style>

<script setup lang="ts">
import { onMounted, onUnmounted, ref, computed } from 'vue';
import { NCard, NIcon, NProgress, NSelect } from 'naive-ui';
import { CloseOutline, PulseOutline, SettingsOutline } from '@vicons/ionicons5';
import TerminalSetting from './TerminalSetting.vue';

const showPanel = ref(false);
const showSettings = ref(false);
const timer = ref<NodeJS.Timeout | null>(null);

const emit = defineEmits<{
  (e: 'updateSettings', settings: any): void;
}>();

const props = defineProps<{
  hostname: string;
  port: number;
  username: string;
  password?: string;
  initialSettings?: {
    fontSize: number;
    background: string;
    foreground: string;
  };
}>();

const handleUpdateSettings = (newSettings: any) => {
  emit('updateSettings', newSettings);
};

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
  network: {
    rx_rate: number;
    tx_rate: number;
  };
  interfaces: Array<{
    name: string;
    rx_rate: number;
    tx_rate: number;
  }>;
}

const stats = ref<SystemStats>({
  cpu: 0,
  memory: { used: 0, total: 0, usage: 0 },
  swap: { used: 0, total: 0, usage: 0 },
  network: { rx_rate: 0, tx_rate: 0 },
  interfaces: []
});

const selectedInterface = ref<string>('total');

const interfaceOptions = computed(() => {
  const list = stats.value.interfaces
    .filter(i => i.name !== 'lo') // Exclude loopback
    .map(i => ({
      label: i.name,
      value: i.name
    }));
  return [{ label: 'Total', value: 'total' }, ...list];
});

const currentNetworkStats = computed(() => {
  if (selectedInterface.value === 'total') {
    return stats.value.network;
  }
  const iface = stats.value.interfaces.find(i => i.name === selectedInterface.value);
  return iface ? { rx_rate: iface.rx_rate, tx_rate: iface.tx_rate } : { rx_rate: 0, tx_rate: 0 };
});

const buildServiceUrl = (path: string) => {
  const base = import.meta.env.VITE_SERVICE_BASE_URL as string | undefined;
  if (base) {
    const trimmed = base.endsWith('/') ? base.slice(0, -1) : base;
    return `${trimmed}${path}`;
  }
  const protocol = window.location.protocol;
  const host = window.location.host;
  const httpProto = protocol === 'https:' ? 'https:' : 'http:';
  return `${httpProto}//${host}${path}`;
};

const formatBytes = (bytes: number, decimals = 1) => {
  if (bytes === 0) return '0 B';
  const k = 1024;
  const dm = decimals < 0 ? 0 : decimals;
  const sizes = ['B', 'KB', 'MB', 'GB', 'TB', 'PB', 'EB', 'ZB', 'YB'];
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  return parseFloat((bytes / Math.pow(k, i)).toFixed(dm)) + ' ' + sizes[i];
};

const fetchStats = async () => {
  if (!props.hostname || !props.username) return;
  try {
    const url = buildServiceUrl('/api/ssh/monitor');
    const res = await fetch(url, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        hostname: props.hostname,
        port: props.port,
        username: props.username,
        password: props.password ?? ''
      })
    });
    if (!res.ok) throw new Error(`request failed: ${res.status}`);
    const data = (await res.json()) as SystemStats;
    stats.value = {
      ...data,
      network: data.network || { rx_rate: 0, tx_rate: 0 },
      interfaces: data.interfaces || []
    };
    
    // 如果是第一次加载且当前选的是 'total'，可以尝试自动选中第一个非 lo 网卡
    // 不过用户可能更喜欢 Total，暂时保留 Total 为默认
    // 如果想要默认选中第一个物理网卡，可以在这里判断 selectedInterface.value === 'total' 并切换
  } catch {
    // 静默失败，保留上次数据
  }
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
  <div class="pointer-events-none absolute inset-0 z-[100] h-full w-full">
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

        <div class="mt-2 space-y-3">
          <!-- CPU -->
          <div class="flex flex-col gap-1">
            <div class="flex justify-between text-xs text-gray-100">
              <span>CPU</span>
              <span>{{ stats.cpu }}%</span>
            </div>
            <NProgress
              type="line"
              :percentage="stats.cpu"
              :color="stats.cpu > 80 ? '#d03050' : '#18a058'"
              :height="8"
              :show-indicator="false"
              processing
            />
          </div>

          <!-- Memory -->
          <div class="flex flex-col gap-1">
            <div class="flex justify-between text-xs text-gray-100">
              <span>MEM</span>
              <span>{{ formatBytes(stats.memory.used) }} / {{ formatBytes(stats.memory.total) }}</span>
            </div>
            <NProgress
              type="line"
              :percentage="stats.memory.usage"
              :show-indicator="false"
              :color="stats.memory.usage > 80 ? '#d03050' : '#2080f0'"
              :height="8"
            />
          </div>

          <!-- Swap -->
          <div class="flex flex-col gap-1">
            <div class="flex justify-between text-xs text-gray-100">
              <span>SWAP</span>
              <span>{{ formatBytes(stats.swap.used) }} / {{ formatBytes(stats.swap.total) }}</span>
            </div>
            <NProgress
              type="line"
              :percentage="stats.swap.usage"
              :show-indicator="false"
              :color="stats.swap.usage > 80 ? '#d03050' : '#f0a020'"
              :height="8"
            />
          </div>
          <!-- Network -->
          <div class="flex flex-col gap-1">
            <div class="flex items-center justify-between text-xs text-gray-100">
              <span>Network</span>
              <!-- <NSelect
                v-model:value="selectedInterface"
                :options="interfaceOptions"
                size="tiny"
                class="w-24"
                :consistent-menu-width="false"
              /> -->
            </div>
            <div class="flex justify-between text-xs text-gray-300">
              <span class="flex items-center gap-1">
                <span class="i-carbon-arrow-down text-green-400">↓</span>
                {{ formatBytes(currentNetworkStats.rx_rate) }}/s
              </span>
              <span class="flex items-center gap-1">
                <span class="i-carbon-arrow-up text-blue-400">↑</span>
                {{ formatBytes(currentNetworkStats.tx_rate) }}/s
              </span>
            </div>
          </div>
        </div>
      </NCard>
    </div>

    <!-- Settings Panel: Top Right (replaces Stats Panel) -->
    <TerminalSetting
      v-if="showSettings"
      :initial-settings="props.initialSettings"
      @update-settings="handleUpdateSettings"
      @close="showSettings = false"
    />

    <!-- Buttons: Bottom Right -->
    <div class="pointer-events-auto absolute bottom-2 right-2 z-10 flex flex-col items-end gap-1">
      <!-- Toggle Button -->
      <div
        class="cursor-pointer bg-black bg-opacity-50 p-1 text-white transition-colors hover:bg-opacity-70"
        @click="togglePanel"  style="height: 28px;"
      >
        <NIcon size="20"><PulseOutline /></NIcon>
      </div>

      <!-- Settings Button -->
      <div
        class="cursor-pointer bg-black bg-opacity-50 p-1 text-white transition-colors hover:bg-opacity-70"
        @click="showSettings = !showSettings" style="height: 28px ;"
      >
        <NIcon size="20"><SettingsOutline /></NIcon>
      </div>
    </div>
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

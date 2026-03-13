<script setup lang="ts">
import { computed, defineAsyncComponent, onMounted, reactive, ref } from 'vue';
import { useMessage } from 'naive-ui';
import { request } from '@/service/request';
import XModal from '@/components/xmodal/index.vue';
import Group from './modules/Group.vue';
import Info from './modules/Info.vue';
import type { ConnectionInfo } from './modules/Info.vue';
import FileManager from './modules/FileManager.vue';
import ServerMonitor from './modules/ServerMonitor.vue';
// 懒加载终端组件
const Terminal = defineAsyncComponent(() => import('./modules/Terminal.vue'));

interface Session {
  id: string;
  connection: any; // Connection details
  status: any; // Connection status
}

interface DetachedSession {
  id: string;
  show: boolean;
  x: number;
  y: number;
  width: number;
  height: number;
  lastFocusedAt: number;
}

const sessions = ref<Session[]>([]);
const activeSessionId = ref<string | null>(null);
const detachedSessions = ref<DetachedSession[]>([]);

const terminalRefs = ref<Record<string, any>>({});
const fileManagerRefs = ref<Record<string, any>>({});

const message = useMessage();

const terminalSettings = reactive({
  fontSize: 14,
  background: '#1e1e1e',
  foreground: '#d4d4d4',
  cursor: '#ffffff',
  cursorStyle: 'block' as 'block' | 'underline' | 'bar',
  cursorBlink: true
});

onMounted(async () => {
  // Load terminal config from backend
  try {
    const { data } = await request<{ config: string }>({
      url: '/api/user/terminal-config',
      method: 'get'
    });
    if (data?.config) {
      const config = JSON.parse(data.config);
      if (config.fontSize) terminalSettings.fontSize = config.fontSize;
      if (config.theme) {
        if (config.theme.background) terminalSettings.background = config.theme.background;
        if (config.theme.foreground) terminalSettings.foreground = config.theme.foreground;
        if (config.theme.cursor) terminalSettings.cursor = config.theme.cursor;
      }
      if (config.cursorStyle) terminalSettings.cursorStyle = config.cursorStyle;
      if (typeof config.cursorBlink === 'boolean') terminalSettings.cursorBlink = config.cursorBlink;
    }
  } catch (e) {
    // console.error('Failed to load terminal config:', e);
  }
});

const activeSession = computed(() => {
  return sessions.value.find(s => s.id === activeSessionId.value) || null;
});

const isDetached = (id: string) => detachedSessions.value.some(s => s.id === id && s.show);

const scheduleFocus = (id: string) => {
  const delays = [0, 80, 200, 400];
  delays.forEach(delay => {
    setTimeout(() => {
      const terminal = terminalRefs.value[id];
      if (terminal && terminal.focus) {
        terminal.focus();
      }
    }, delay);
  });
};

const topModalSessionId = computed(() => {
  const visible = detachedSessions.value.filter(s => s.show);
  if (visible.length === 0) return null;
  return visible.reduce((a, b) => (a.lastFocusedAt >= b.lastFocusedAt ? a : b)).id;
});

const fileListSessionId = computed(() => {
  return topModalSessionId.value ?? activeSessionId.value;
});

const handleConnect = async (form: any) => {
  // Create new session
  const sessionId = Date.now().toString();
  const newSession: Session = {
    id: sessionId,
    connection: form,
    status: { connected: false }
  };

  sessions.value.push(newSession);
  activeSessionId.value = sessionId;
};

const handleStatusChange = (sessionId: string, status: any) => {
  const session = sessions.value.find(s => s.id === sessionId);
  if (session) {
    session.status = status;
  }
};

const handleSettingsUpdate = async (settings: any) => {
  // Update local state
  if (settings.fontSize) terminalSettings.fontSize = settings.fontSize;
  if (settings.theme) {
    if (settings.theme.background) terminalSettings.background = settings.theme.background;
    if (settings.theme.foreground) terminalSettings.foreground = settings.theme.foreground;
    if (settings.theme.cursor) terminalSettings.cursor = settings.theme.cursor;
  }
  if (settings.cursorStyle) terminalSettings.cursorStyle = settings.cursorStyle;
  if (typeof settings.cursorBlink === 'boolean') terminalSettings.cursorBlink = settings.cursorBlink;

  // Update active terminals
  Object.values(terminalRefs.value).forEach((terminal: any) => {
    if (terminal && terminal.setOptions) {
      terminal.setOptions(settings);
    }
  });

  // Save to backend
  try {
    await request({
      url: '/api/user/terminal-config',
      method: 'post',
      data: {
        config: JSON.stringify({
          fontSize: terminalSettings.fontSize,
          theme: {
            background: terminalSettings.background,
            foreground: terminalSettings.foreground,
            cursor: terminalSettings.cursor
          },
          cursorStyle: terminalSettings.cursorStyle,
          cursorBlink: terminalSettings.cursorBlink
        })
      }
    });
    // message.success('Settings saved');
  } catch (e) {
    message.error('Failed to save settings');
  }
};

const handleCloseTab = (sessionId: string) => {
  // Disconnect
  if (terminalRefs.value[sessionId]) {
    terminalRefs.value[sessionId].disconnect();
  }

  // Remove session
  const index = sessions.value.findIndex(s => s.id === sessionId);
  if (index !== -1) {
    sessions.value.splice(index, 1);

    // Update active session if needed
    if (activeSessionId.value === sessionId) {
      if (sessions.value.length > 0) {
        // Switch to the nearest tab (previous one or the new first one)
        activeSessionId.value = sessions.value[Math.max(0, index - 1)].id;
      } else {
        activeSessionId.value = null;
      }
    }
  }

  // Clean up refs
  terminalRefs.value = Object.fromEntries(Object.entries(terminalRefs.value).filter(([id]) => id !== sessionId));
  fileManagerRefs.value = Object.fromEntries(Object.entries(fileManagerRefs.value).filter(([id]) => id !== sessionId));
  detachedSessions.value = detachedSessions.value.filter(s => s.id !== sessionId);
};

const handleCwdChange = (sessionId: string, path: string) => {
  if (fileManagerRefs.value[sessionId]) {
    fileManagerRefs.value[sessionId].navigateTo(path);
  }
};

const getDetachedSession = (id: string) => {
  return detachedSessions.value.find(s => s.id === id) || null;
};

const isModalVisible = (id: string) => {
  return activeSessionId.value === id || isDetached(id);
};

const updateDetachedSession = (id: string, patch: Partial<DetachedSession>) => {
  const modal = detachedSessions.value.find(s => s.id === id);
  if (modal) {
    Object.assign(modal, patch);
  }
};

const connectionsForInfo = computed<ConnectionInfo[]>(() => {
  return sessions.value.map(s => ({
    id: s.id,
    username: s.connection.username,
    hostname: s.connection.hostname,
    port: s.connection.port
  }));
});

const ensureActiveSession = () => {
  if (activeSessionId.value && !isDetached(activeSessionId.value)) return;
  const next = sessions.value.find(s => !isDetached(s.id));
  activeSessionId.value = next ? next.id : null;
};

const getSessionTitle = (id: string) => {
  const session = sessions.value.find(s => s.id === id);
  if (!session) return 'SSH';
  return `${session.connection.username}@${session.connection.hostname}`;
};

const handleDetach = async (payload: { id: string; x: number; y: number }) => {
  const { id, x, y } = payload;
  const existing = detachedSessions.value.find(s => s.id === id);
  const width = 900;
  const height = 520;
  const safeX = Math.max(0, x - Math.round(width / 3));
  const safeY = Math.max(0, y - 40);
  const now = Date.now();

  if (existing) {
    existing.show = true;
    existing.x = safeX;
    existing.y = safeY;
    existing.width = width;
    existing.height = height;
    existing.lastFocusedAt = now;
  } else {
    detachedSessions.value.push({
      id,
      show: true,
      x: safeX,
      y: safeY,
      width,
      height,
      lastFocusedAt: now
    });
  }

  ensureActiveSession();
  scheduleFocus(id);
};

const handleModalClose = (id: string) => {
  detachedSessions.value = detachedSessions.value.filter(s => s.id !== id);
  if (sessions.value.some(s => s.id === id)) {
    activeSessionId.value = id;
    scheduleFocus(id);
  }
};

const handleModalFocus = (id: string) => {
  const modal = detachedSessions.value.find(s => s.id === id);
  if (modal) {
    modal.lastFocusedAt = Date.now();
    scheduleFocus(id);
  }
};
</script>

<template>
  <div class="h-full w-full overflow-hidden">
    <NSplit direction="horizontal" :default-size="0.1" :min="0.1" :max="0.2">
      <template #1>
        <div class="h-full overflow-auto border-r border-gray-200 dark:border-gray-700">
          <Group @connect="handleConnect" />
        </div>
      </template>
      <template #2>
        <NSplit direction="vertical" :default-size="0.8" :min="0.2" :max="0.8">
          <template #1>
            <div class="h-full flex flex-col overflow-hidden p-4px">
              <div class="border-gray-200 dark:border-gray-700" style="border-bottom: 1px solid;">
                <Info
                  v-model:active-id="activeSessionId"
                  :connections="connectionsForInfo"
                  @close="handleCloseTab"
                  @detach="handleDetach"
                />
              </div>
              <div class="relative flex-1 overflow-hidden">
                <div
                  v-if="sessions.length === 0"
                  class="absolute inset-0 flex items-center justify-center text-gray-400"
                >
                  Select a server to connect
                </div>

                <div v-for="session in sessions" :key="`local-wrapper-${session.id}`" class="h-full w-full">
                  <XModal
                    card-content-padding="0px"
                    :show="isModalVisible(session.id)"
                    :title="getSessionTitle(session.id)"
                    :x="getDetachedSession(session.id)?.x ?? 0"
                    :y="getDetachedSession(session.id)?.y ?? 0"
                    :width="getDetachedSession(session.id)?.width ?? 900"
                    :height="getDetachedSession(session.id)?.height ?? 520"
                    :inline="!isDetached(session.id)"
                    :show-header="isDetached(session.id)"
                    :shadow="isDetached(session.id)"
                    :draggable="isDetached(session.id)"
                    :resizable="isDetached(session.id)"
                    @update:x="
                      val => {
                        updateDetachedSession(session.id, { x: val });
                      }
                    "
                    @update:y="
                      val => {
                        updateDetachedSession(session.id, { y: val });
                      }
                    "
                    @update:width="
                      val => {
                        updateDetachedSession(session.id, { width: val });
                        scheduleFocus(session.id);
                      }
                    "
                    @update:height="
                      val => {
                        updateDetachedSession(session.id, { height: val });
                        scheduleFocus(session.id);
                      }
                    "
                    @update:show="
                      val => {
                        if (!val) handleModalClose(session.id);
                      }
                    "
                    @focus="() => handleModalFocus(session.id)"
                  >
                    <Suspense>
                      <template #default>
                        <Terminal
                          :ref="
                            el => {
                              if (el) terminalRefs[session.id] = el;
                            }
                          "
                          :hostname="session.connection.hostname"
                          :username="session.connection.username"
                          :password="session.connection.password"
                          :port="session.connection.port"
                          :auto-connect="true"
                          :theme="{
                            background: terminalSettings.background,
                            foreground: terminalSettings.foreground,
                            cursor: terminalSettings.cursor
                          }"
                          :font-size="terminalSettings.fontSize"
                          :cursor-style="terminalSettings.cursorStyle"
                          :cursor-blink="terminalSettings.cursorBlink"
                          @status-change="status => handleStatusChange(session.id, status)"
                          @cwd-change="path => handleCwdChange(session.id, path)"
                        />
                      </template>
                      <template #fallback>
                        <div class="h-full flex items-center justify-center">
                          <NSpin size="large" />
                        </div>
                      </template>
                    </Suspense>
                  </XModal>
                </div>

                <ServerMonitor
                  v-if="activeSession?.status?.connected"
                  class="pointer-events-none absolute inset-0 z-[100]"
                  :hostname="activeSession?.connection?.hostname"
                  :port="activeSession?.connection?.port"
                  :username="activeSession?.connection?.username"
                  :password="activeSession?.connection?.password"
                  :initial-settings="terminalSettings"
                  @update-settings="handleSettingsUpdate"
                />
              </div>
            </div>
          </template>
          <template #2>
            <div class="relative h-full overflow-hidden border-t border-gray-200 dark:border-gray-700">
              <div v-if="sessions.length === 0" class="absolute inset-0 flex items-center justify-center text-gray-400">
                No active session
              </div>
              <div
                v-for="session in sessions"
                v-show="fileListSessionId === session.id"
                :key="`file-${session.id}`"
                class="h-full w-full"
              >
                <FileManager
                  :ref="
                    el => {
                      if (el) fileManagerRefs[session.id] = el;
                    }
                  "
                  :hostname="session.connection.hostname"
                  :username="session.connection.username"
                  :password="session.connection.password"
                  :port="session.connection.port"
                  :auto-connect="true"
                />
              </div>
            </div>
          </template>
        </NSplit>
      </template>
    </NSplit>
  </div>
</template>

<style scoped></style>

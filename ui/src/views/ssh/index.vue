<script setup lang="ts">
import { computed, defineAsyncComponent, nextTick, reactive, ref } from 'vue';
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

const sessions = ref<Session[]>([]);
const activeSessionId = ref<string | null>(null);

const terminalRefs = ref<Record<string, any>>({});
const fileManagerRefs = ref<Record<string, any>>({});

const terminalSettings = reactive({
  fontSize: 14,
  background: '#1e1e1e',
  foreground: '#d4d4d4'
});

const activeSession = computed(() => {
  return sessions.value.find(s => s.id === activeSessionId.value) || null;
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

  // Wait for DOM update to ensure refs are available
  await nextTick();

  // FileManager will auto-connect via onMounted if props are set
};

const handleStatusChange = (sessionId: string, status: any) => {
  const session = sessions.value.find(s => s.id === sessionId);
  if (session) {
    session.status = status;
  }
};

const handleSettingsUpdate = (settings: any) => {
  if (settings.fontSize) terminalSettings.fontSize = settings.fontSize;
  if (settings.theme) {
    if (settings.theme.background) terminalSettings.background = settings.theme.background;
    if (settings.theme.foreground) terminalSettings.foreground = settings.theme.foreground;
  }

  Object.values(terminalRefs.value).forEach((terminal: any) => {
    if (terminal && terminal.setOptions) {
      terminal.setOptions(settings);
    }
  });
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
};

const handleCwdChange = (sessionId: string, path: string) => {
  if (fileManagerRefs.value[sessionId]) {
    fileManagerRefs.value[sessionId].navigateTo(path);
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
            <div class="h-full flex flex-col overflow-hidden">
              <div class="border-gray-200 dark:border-gray-700">
                <Info v-model:active-id="activeSessionId" :connections="connectionsForInfo" @close="handleCloseTab" />
              </div>
              <div class="relative flex-1 overflow-hidden">
                <!-- Empty State -->
                <div
                  v-if="sessions.length === 0"
                  class="absolute inset-0 flex items-center justify-center text-gray-400"
                >
                  Select a server to connect
                </div>

                <!-- Terminals -->
                <div
                  v-for="session in sessions"
                  v-show="activeSessionId === session.id"
                  :key="session.id"
                  class="h-full w-full"
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
                </div>

                <ServerMonitor
                  v-if="activeSession?.status?.connected"
                  class="pointer-events-none absolute inset-0 z-[100]"
                  :session="activeSession"
                  :initial-settings="terminalSettings"
                  @update-settings="handleSettingsUpdate"
                />
              </div>
            </div>
          </template>
          <template #2>
            <div class="relative h-full overflow-hidden border-t border-gray-200 dark:border-gray-700">
              <!-- Empty State -->
              <div v-if="sessions.length === 0" class="absolute inset-0 flex items-center justify-center text-gray-400">
                No active session
              </div>

              <!-- File Managers -->
              <div
                v-for="session in sessions"
                v-show="activeSessionId === session.id"
                :key="session.id"
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

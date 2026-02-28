<script setup lang="ts">
import { nextTick, onMounted, onUnmounted, reactive, ref } from 'vue';
import { useDebounceFn, useResizeObserver } from '@vueuse/core';
import { useMessage } from 'naive-ui';
import { FitAddon } from '@xterm/addon-fit';
import { SearchAddon } from '@xterm/addon-search';
import { Terminal } from 'xterm';
import 'xterm/css/xterm.css';

interface ConnectionDetails {
  hostname: string;
  port: number;
  username: string;
  password?: string;
}

const props = withDefaults(
  defineProps<{
    hostname?: string;
    port?: number;
    username?: string;
    password?: string;
    autoConnect?: boolean;
  }>(),
  {
    port: 22,
    autoConnect: false
  }
);

const emit = defineEmits<{
  (e: 'statusChange', status: any): void;
  (e: 'cwd-change', path: string): void;
}>();

// Use message hook
const message = useMessage();

// UI State
const isConnected = ref(false);
const isConnecting = ref(false);
const showConnectionForm = ref(false);
const connectionForm = reactive<ConnectionDetails>({
  hostname: props.hostname || '',
  port: props.port || 22,
  username: props.username || '',
  password: props.password || ''
});

// Terminal refs
const terminalContainer = ref<HTMLDivElement | null>(null);
const terminalInstance = ref<Terminal | null>(null);
const fitAddon = ref<FitAddon | null>(null);
const searchAddon = ref<SearchAddon | null>(null);

// WebSocket ref
const socket = ref<WebSocket | null>(null);

// Resize handling
const handleResize = useDebounceFn(() => {
  if (
    terminalContainer.value &&
    terminalContainer.value.clientWidth > 0 &&
    terminalContainer.value.clientHeight > 0 &&
    fitAddon.value
  ) {
    try {
      fitAddon.value.fit();
    } catch (e) {
      console.error('Failed to fit terminal:', e);
    }
  }
}, 20);

useResizeObserver(terminalContainer, handleResize);

function parseCdPathFromLine(line: string): string | null {
  const matches = line.match(/(?:^|[\s;&|])cd\s+([^\s;&|]+)/g);
  if (!matches?.length) return null;

  const lastMatch = matches[matches.length - 1];
  const pathMatch = lastMatch.match(/cd\s+([^\s;&|]+)/);
  if (!pathMatch) return null;

  const path = pathMatch[1].replace(/["']/g, '');
  return path || null;
}

function tryEmitCwdChange() {
  const term = terminalInstance.value;
  if (!term) return;

  const buffer = term.buffer.active;
  const currentLine = buffer.getLine(buffer.cursorY)?.translateToString(true).trim();
  if (!currentLine) return;

  const path = parseCdPathFromLine(currentLine);
  if (!path) return;

  emit('cwd-change', path);
}

// Initialize terminal
const initTerminal = () => {
  if (terminalInstance.value) return;

  const terminal = new Terminal({
    cursorBlink: true,
    cursorStyle: 'block',
    fontSize: 16,
    fontFamily: 'Consolas, "Courier New", monospace',
    theme: {
      background: '#1e1e1e',
      foreground: '#d4d4d4',
      cursor: '#ffffff',
      selectionBackground: '#ffffff40',
      black: '#000000',
      red: '#cd0000',
      green: '#00cd00',
      yellow: '#cdcd00',
      blue: '#0000ee',
      magenta: '#cd00cd',
      cyan: '#00cdcd',
      white: '#e5e5e5',
      brightBlack: '#7f7f7f',
      brightRed: '#ff0000',
      brightGreen: '#00ff00',
      brightYellow: '#ffff00',
      brightBlue: '#5c5cff',
      brightMagenta: '#ff00ff',
      brightCyan: '#00ffff',
      brightWhite: '#ffffff'
    },
    allowTransparency: false,
    convertEol: true,
    scrollback: 1000,
    tabStopWidth: 4
  });

  const fit = new FitAddon();
  const search = new SearchAddon();

  terminal.loadAddon(fit);
  terminal.loadAddon(search);

  terminalInstance.value = terminal;
  fitAddon.value = fit;
  searchAddon.value = search;

  if (terminalContainer.value) {
    terminal.open(terminalContainer.value);
    try {
      fit.fit();
    } catch (e) {
      console.warn('Initial fit failed:', e);
    }
  }

  terminal.onData(data => {
    if (socket.value && socket.value.readyState === WebSocket.OPEN) {
      // Send input to server
      const msg = { type: 'input', data };
      socket.value.send(JSON.stringify(msg));
    }

    if (data === '\r') {
      try {
        tryEmitCwdChange();
      } catch (e) {
        console.error('Error parsing command:', e);
      }
    }
  });

  terminal.onResize(size => {
    if (socket.value && socket.value.readyState === WebSocket.OPEN && size.cols > 0 && size.rows > 0) {
      const msg = { type: 'resize', width: size.cols, height: size.rows };
      socket.value.send(JSON.stringify(msg));
    }
  });
};
const handleMessage = (msg: any) => {
  switch (msg.type) {
    case 'connected': {
      isConnected.value = true;
      isConnecting.value = false;
      terminalInstance.value?.writeln(`\r\n\x1B[32mSSH Connected.\x1B[0m\r\n`);
      nextTick(() => {
        terminalInstance.value?.focus();
        fitAddon.value?.fit();
      });
      emit('statusChange', {
        connected: true,
        username: connectionForm.username,
        hostname: connectionForm.hostname,
        port: connectionForm.port
      });
      break;
    }
    case 'output': {
      const text = msg.data || msg.content;
      if (text) terminalInstance.value?.write(text);
      break;
    }
    case 'error': {
      const errorText = msg.message || 'Unknown error';
      terminalInstance.value?.writeln(`\r\n\x1B[31mError: ${errorText}\x1B[0m`);
      message.error(errorText);
      isConnecting.value = false;
      break;
    }
    case 'disconnected': {
      break;
    }
    default: {
      break;
    }
  }
};

const connect = (details?: ConnectionDetails) => {
  if (isConnecting.value || isConnected.value) return;

  if (details) {
    Object.assign(connectionForm, details);
  }

  if (!connectionForm.hostname || !connectionForm.username) {
    message.error('Hostname and Username are required');
    return;
  }

  isConnecting.value = true;
  showConnectionForm.value = false;

  if (!terminalInstance.value) {
    initTerminal();
  }

  terminalInstance.value?.clear();
  terminalInstance.value?.writeln(
    `\x1B[36mConnecting to ${connectionForm.username}@${connectionForm.hostname}:${connectionForm.port}...\x1B[0m`
  );

  // WebSocket URL construction
  const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
  const serviceBaseUrl = import.meta.env.VITE_SERVICE_BASE_URL;
  let wsUrl = '';

  if (serviceBaseUrl) {
    const baseUrl = serviceBaseUrl.endsWith('/') ? serviceBaseUrl.slice(0, -1) : serviceBaseUrl;
    wsUrl = `${baseUrl.replace(/^http/, 'ws')}/ws/ssh-pty`;
  } else {
    const host = window.location.host;
    wsUrl = `${protocol}//${host}/ws/ssh-pty`;
  }

  const ws = new WebSocket(wsUrl);
  socket.value = ws;

  ws.onopen = () => {
    terminalInstance.value?.writeln(`\x1B[32mWebSocket Connected. Negotiating SSH...\x1B[0m`);

    const cols = terminalInstance.value?.cols || 80;
    const rows = terminalInstance.value?.rows || 24;

    const msg = {
      type: 'connect',
      credentials: {
        hostname: connectionForm.hostname,
        port: Number(connectionForm.port),
        username: connectionForm.username,
        password: connectionForm.password
      },
      col_width: cols,
      row_height: rows
    };
    ws.send(JSON.stringify(msg));
  };

  ws.onmessage = event => {
    try {
      const msg = JSON.parse(event.data);
      handleMessage(msg);
    } catch {
      terminalInstance.value?.write(event.data);
    }
  };

  ws.onclose = () => {
    isConnected.value = false;
    isConnecting.value = false;
    terminalInstance.value?.writeln(`\r\n\x1B[33mConnection closed.\x1B[0m`);
    socket.value = null;
    showConnectionForm.value = true;
    emit('statusChange', { connected: false });
  };

  ws.onerror = err => {
    isConnecting.value = false;
    terminalInstance.value?.writeln(`\r\n\x1B[31mConnection error.\x1B[0m`);
    console.error(err);
    emit('statusChange', { connected: false });
  };
};

const disconnect = () => {
  if (socket.value) {
    socket.value.close();
  }
  emit('statusChange', { connected: false });
};

onMounted(() => {
  if (props.autoConnect && props.hostname && props.username) {
    connect();
  } else if (props.hostname || props.username) {
    // If props are provided but not autoConnect, just fill form
  }

  initTerminal();
});

onUnmounted(() => {
  disconnect();
  terminalInstance.value?.dispose();
});

const setOptions = (options: any) => {
  if (!terminalInstance.value) return;

  Object.entries(options).forEach(([key, value]) => {
    if (key === 'theme' && typeof value === 'object') {
      const currentTheme = terminalInstance.value!.options.theme || {};
      terminalInstance.value!.options.theme = { ...currentTheme, ...value };
    } else {
      (terminalInstance.value!.options as Record<string, unknown>)[key] = value;
    }
  });

  // Re-fit if font-related settings changed
  if (options.fontSize || options.fontFamily || options.lineHeight) {
    nextTick(() => {
      fitAddon.value?.fit();
    });
  }
};

defineExpose({
  connect,
  disconnect,
  terminal: terminalInstance,
  setOptions
});
</script>

<template>
  <div class="relative h-full flex flex-col overflow-hidden bg-[#1e1e1e]">
    <!-- Terminal Container -->
    <div ref="terminalContainer" class="relative w-full flex-1 overflow-hidden"></div>
  </div>
</template>

<style scoped>
:deep(.xterm-viewport) {
  overflow-y: auto;
}
</style>

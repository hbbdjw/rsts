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

defineOptions({
  name: 'SshTerminal'
});

const props = withDefaults(
  defineProps<{
    hostname?: string;
    port?: number;
    username?: string;
    password?: string;
    autoConnect?: boolean;
    cursorBlink?: boolean;
    cursorStyle?: 'block' | 'underline' | 'bar';
    theme?: Record<string, string>;
    fontSize?: number;
  }>(),
  {
    hostname: '',
    port: 22,
    username: '',
    password: '',
    autoConnect: false,
    cursorBlink: true,
    cursorStyle: 'block',
    theme: () => ({}),
    fontSize: 16
  }
);

const emit = defineEmits<{
  (e: 'statusChange', status: any): void;
  (e: 'cwdChange', path: string): void;
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
const backgroundColor = ref(props.theme?.background || '#1e1e1e');

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
      // Force focus after resize to ensure input works
      if (terminalInstance.value) {
        terminalInstance.value.focus();
      }
    } catch {}
  }
}, 50);

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

  emit('cwdChange', path);
}

// Initialize terminal
const initTerminal = () => {
  if (terminalInstance.value) return;

  const terminal = new Terminal({
    cursorBlink: props.cursorBlink,
    cursorStyle: props.cursorStyle,
    fontSize: props.fontSize,
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
      brightWhite: '#ffffff',
      ...props.theme
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
    } catch {}
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
      } catch {}
    }
  });

  terminal.onResize(size => {
    // console.log('Terminal resized:', size);
    if (size.cols <= 0 || size.rows <= 0 || Number.isNaN(size.cols) || Number.isNaN(size.rows)) return;

    if (socket.value && socket.value.readyState === WebSocket.OPEN) {
      const msg = { type: 'resize', width: size.cols, height: size.rows };
      socket.value.send(JSON.stringify(msg));
    }
  });
};

function focus() {
  nextTick(() => {
    if (!terminalInstance.value || !fitAddon.value) return;

    const container = terminalContainer.value;
    if (!container) return;

    if (container.clientWidth > 0 && container.clientHeight > 0) {
      try {
        fitAddon.value.fit();
      } catch {}

      const textarea = terminalInstance.value.element?.querySelector(
        '.xterm-helper-textarea'
      ) as HTMLTextAreaElement | null;

      if (textarea) {
        textarea.focus();
      }

      terminalInstance.value.focus();

      const cols = terminalInstance.value.cols;
      const rows = terminalInstance.value.rows;
      if (cols > 0 && rows > 0 && socket.value && socket.value.readyState === WebSocket.OPEN) {
        const msg = { type: 'resize', width: cols, height: rows };
        socket.value.send(JSON.stringify(msg));
      }
    } else {
      setTimeout(focus, 100);
    }
  });
  // nextTick(() => {
  //   if (terminalContainer.value) {
  //     const currentParent = terminalInstance.value?.element?.parentElement;
  //     if (currentParent && currentParent !== terminalContainer.value) {
  //       terminalInstance.value?.dispose();
  //       terminalInstance.value = null;
  //       fitAddon.value = null;
  //       searchAddon.value = null;
  //       terminalContainer.value.innerHTML = '';
  //       initTerminal();
  //     }
  //   }

  //   if (!terminalInstance.value || !fitAddon.value || !terminalContainer.value) return;

  //   if (terminalContainer.value.clientWidth <= 0 || terminalContainer.value.clientHeight <= 0) {
  //     setTimeout(focus, 100);
  //     return;
  //   }

  //   try {
  //     fitAddon.value.fit();
  //   } catch {}

  //   const cols = terminalInstance.value.cols;
  //   const rows = terminalInstance.value.rows;

  //   if (cols > 0 && rows > 0) {
  //     terminalInstance.value.focus();
  //     const textarea = terminalInstance.value.element?.querySelector('textarea') as HTMLTextAreaElement | null;
  //     textarea?.focus();
  //     terminalInstance.value.refresh(0, rows - 1);

  //     if (socket.value && socket.value.readyState === WebSocket.OPEN) {
  //       const msg = { type: 'resize', width: cols, height: rows };
  //       socket.value.send(JSON.stringify(msg));
  //     }
  //   } else {
  //     setTimeout(focus, 100);
  //   }
  // });
}

const handleMessage = (msg: any) => {
  switch (msg.type) {
    case 'connected': {
      isConnected.value = true;
      isConnecting.value = false;
      terminalInstance.value?.writeln(`\r\n\x1B[32mSSH Connected.\x1B[0m\r\n`);
      nextTick(() => {
        focus();
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

  ws.onerror = () => {
    isConnecting.value = false;
    terminalInstance.value?.writeln(`\r\n\x1B[31mConnection error.\x1B[0m`);
    message.error('Connection error');
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
      if ((value as any).background) {
        backgroundColor.value = (value as any).background;
      }
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
  setOptions,
  focus
});
</script>

<template>
  <div class="relative h-full flex flex-col overflow-hidden" :style="{ backgroundColor: backgroundColor }">
    <div
      ref="terminalContainer"
      class="relative w-full flex-1 overflow-hidden"
      @mousedown="focus"
      @touchstart="focus"
    ></div>
  </div>
</template>

<style scoped>
:deep(.xterm-viewport) {
  overflow-y: auto;
}
</style>

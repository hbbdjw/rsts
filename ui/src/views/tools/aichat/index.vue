<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, reactive, ref, watch } from 'vue';
import { useMessage } from 'naive-ui';
import { marked } from 'marked';
import { markedHighlight } from 'marked-highlight';
import hljs from 'highlight.js';
import 'highlight.js/styles/github.css';
import DOMPurify from 'dompurify';
import XModal from '@/components/xmodal/index.vue';

defineOptions({
  name: 'ToolsAiChat'
});

type ChatRole = 'system' | 'user' | 'assistant';
type ResponseState = 'idle' | 'streaming' | 'done' | 'interrupted' | 'error';

interface ChatMessage {
  id: string;
  role: ChatRole;
  content: string;
  createdAt: number;
}

interface Attachment {
  id: string;
  name: string;
  dataUrl: string;
}

const message = useMessage();

// Configure marked with highlight.js
marked.use(
  markedHighlight({
    langPrefix: 'hljs language-',
    highlight(code: string, lang?: string) {
      if (lang && hljs.getLanguage(lang)) {
        return hljs.highlight(code, { language: lang, ignoreIllegals: true }).value;
      }
      return hljs.highlightAuto(code).value;
    }
  })
);

// Custom renderer to add copy button
marked.use({
  renderer: {
    code(token: any) {
      const { text, lang: rawLang } = token;
      const lang = (rawLang || '').match(/\S*/)?.[0] || '';
      return `
        <div class="code-block-wrapper relative group my-3">
          <button class="copy-btn absolute top-2 right-2 px-2 py-1 rounded bg-gray-200 dark:bg-gray-700 text-xs text-gray-600 dark:text-gray-300 opacity-0 group-hover:opacity-100 transition-opacity z-10 cursor-pointer hover:bg-gray-300 dark:hover:bg-gray-600">
            复制
          </button>
          <pre class="!my-0 overflow-auto rounded bg-black/5 p-3 dark:bg-white/5"><code class="hljs language-${lang}">${text}</code></pre>
        </div>
      `;
    }
  }
});

marked.setOptions({
  gfm: true,
  breaks: true
});

function renderMarkdown(source: string) {
  let s = source || '';
  const ticks = (s.match(/```/g) || []).length;
  if (ticks % 2 === 1) s += '\n```';
  const html = marked.parse(s) as string;
  return DOMPurify.sanitize(html, { ADD_ATTR: ['class', 'target'], ADD_TAGS: ['button'] });
}

function handleMessageClick(e: MouseEvent) {
  const target = e.target as HTMLElement;
  if (!target) return;
  const btn = target.closest('.copy-btn');
  if (btn) {
    const wrapper = btn.closest('.code-block-wrapper');
    const codeBlock = wrapper?.querySelector('code');
    if (codeBlock) {
      const text = codeBlock.textContent || '';
      navigator.clipboard
        .writeText(text)
        .then(() => {
          message.success('已复制');
        })
        .catch(() => {
          message.error('复制失败');
        });
    }
  }
}

const isConnecting = ref(false);
const isConnected = ref(false);
const socket = ref<WebSocket | null>(null);

const messages = ref<ChatMessage[]>([]);
const inputText = ref('');
const pending = ref(false);
const currentMessageId = ref<string | null>(null);
let messageBuffer = '';
const responseState = ref<ResponseState>('idle');
let reconnectTimer: number | null = null;
const reconnectAttempts = ref(0);
const manualDisconnect = ref(false);
const showSettings = ref(false);

const attachments = ref<Attachment[]>([]);

const settings = reactive({
  apiKey: localStorage.getItem('tianyi_api_key') || '23147091834c448c9449129961901b32',
  model: localStorage.getItem('tianyi_model_id') || '6d3a57c3a6fb465e968b604783b89eda',
  enableThinking: localStorage.getItem('tianyi_enable_thinking') === 'true' || true,
  temperature: Number(localStorage.getItem('tianyi_temperature') || 1.0),
  topP: Number(localStorage.getItem('tianyi_top_p') || 0.95),
  topK: Number(localStorage.getItem('tianyi_top_k') || 20),
  maxTokens: Number(localStorage.getItem('tianyi_max_tokens') || 2048),
  frequencyPenalty: Number(localStorage.getItem('tianyi_frequency_penalty') || 0.0),
  presencePenalty: Number(localStorage.getItem('tianyi_presence_penalty') || 0.0),
  seed: localStorage.getItem('tianyi_seed') || '',
  stop: localStorage.getItem('tianyi_stop') || ''
});

watch(
  () => ({ ...settings }),
  val => {
    localStorage.setItem('tianyi_api_key', val.apiKey);
    localStorage.setItem('tianyi_model_id', val.model);
    localStorage.setItem('tianyi_enable_thinking', String(val.enableThinking));
    localStorage.setItem('tianyi_temperature', String(val.temperature));
    localStorage.setItem('tianyi_top_p', String(val.topP));
    localStorage.setItem('tianyi_top_k', String(val.topK));
    localStorage.setItem('tianyi_max_tokens', String(val.maxTokens));
    localStorage.setItem('tianyi_frequency_penalty', String(val.frequencyPenalty));
    localStorage.setItem('tianyi_presence_penalty', String(val.presencePenalty));
    localStorage.setItem('tianyi_seed', val.seed);
    localStorage.setItem('tianyi_stop', val.stop);
  },
  { deep: true }
);

const listRef = ref<HTMLElement | null>(null);
const scrollToBottom = async () => {
  await nextTick();
  const el = listRef.value;
  if (!el) return;
  el.scrollTop = el.scrollHeight;
};

watch(
  () => messages.value.length,
  () => {
    scrollToBottom();
  }
);

const wsUrl = computed(() => {
  const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
  const serviceBaseUrl = import.meta.env.VITE_SERVICE_BASE_URL;
  if (serviceBaseUrl) {
    const baseUrl = serviceBaseUrl.endsWith('/') ? serviceBaseUrl.slice(0, -1) : serviceBaseUrl;
    return `${baseUrl.replace(/^http/, 'ws')}/ws/tianyi`;
  }
  return `${protocol}//${window.location.host}/ws/tianyi`;
});

function pushMessage(role: ChatRole, content: string) {
  const id = `${Date.now()}-${Math.random().toString(16).slice(2)}`;
  messages.value.push({
    id,
    role,
    content,
    createdAt: Date.now()
  });
  return id;
}

function handleJsonChunk(chunk: string) {
  try {
    const json = JSON.parse(chunk);
    if (json?.error) {
      const errMsg = json.error?.message || '调用失败';
      pending.value = false;
      responseState.value = 'error';
      message.error(errMsg);
      pushMessage('assistant', `错误：${errMsg}`);
      return true;
    }
    const content = json?.choices?.[0]?.message?.content;
    if (typeof content === 'string') {
      pending.value = false;
      responseState.value = 'done';
      pushMessage('assistant', content);
      return true;
    }
  } catch {}
  return false;
}

function appendToStreamingMessage(text: string) {
  if (!text) return;
  if (!currentMessageId.value) {
    currentMessageId.value = pushMessage('assistant', '');
  }
  const msg = messages.value.find(m => m.id === currentMessageId.value);
  if (!msg) return;
  msg.content += text;
  scrollToBottom();
}

function handleSseData(dataStr: string) {
  if (dataStr === '[DONE]') {
    currentMessageId.value = null;
    pending.value = false;
    responseState.value = 'done';
    return;
  }

  try {
    const json = JSON.parse(dataStr);
    const delta = json.choices?.[0]?.delta;
    const content = delta?.content || '';
    const reasoning = delta?.reasoning_content || '';
    const finish = json.choices?.[0]?.finish_reason;
    const text = `${reasoning}${content}`;
    appendToStreamingMessage(text);
    if (finish) {
      currentMessageId.value = null;
      pending.value = false;
      responseState.value = 'done';
    }
  } catch {}
}

function handleSseChunk(chunk: string) {
  messageBuffer += chunk;
  const lines = messageBuffer.split('\n');
  messageBuffer = lines.pop() || '';

  lines.forEach(line => {
    const trimmed = line.trim();
    if (!trimmed) return;
    if (!trimmed.startsWith('data: ')) return;
    handleSseData(trimmed.slice(6).trim());
  });
}

const responseTag = computed(() => {
  const state = responseState.value;
  if (state === 'streaming') return { type: 'warning' as const, text: '回复中…' };
  if (state === 'done') return { type: 'success' as const, text: '回答完毕' };
  if (state === 'interrupted') return { type: 'error' as const, text: '回复中断' };
  if (state === 'error') return { type: 'error' as const, text: '发生错误' };
  return null;
});

const sendDisabledReason = computed(() => {
  if (!isConnected.value) return 'WebSocket未连接';
  if (pending.value) return '回复中…';
  if (!settings.apiKey.trim()) return '请填写 APP_KEY';
  const content = buildUserContent();
  if (!content) return '';
  return '';
});

function connect() {
  if (isConnecting.value || isConnected.value) return;
  manualDisconnect.value = false;
  isConnecting.value = true;

  const ws = new WebSocket(wsUrl.value);
  socket.value = ws;

  ws.onopen = () => {
    isConnecting.value = false;
    isConnected.value = true;
    reconnectAttempts.value = 0;
    if (reconnectTimer) {
      clearTimeout(reconnectTimer);
      reconnectTimer = null;
    }
  };

  ws.onmessage = evt => {
    const chunk = String(evt.data ?? '');
    if (handleJsonChunk(chunk)) return;
    handleSseChunk(chunk);
  };

  ws.onerror = () => {
    isConnecting.value = false;
    isConnected.value = false;
    pending.value = false;
    if (responseState.value === 'streaming') responseState.value = 'interrupted';
    message.error('WebSocket连接错误');
  };

  ws.onclose = () => {
    isConnecting.value = false;
    isConnected.value = false;
    const wasStreaming = pending.value || responseState.value === 'streaming';
    // const normalClose = e?.code === 1000;
    // const isDone = responseState.value === 'done';
    if (wasStreaming) {
      pending.value = false;
      responseState.value = 'interrupted';
    }
    socket.value = null;
    currentMessageId.value = null;
    messageBuffer = '';

    // 保持常连接：只要不是用户手动断开，始终尝试重连；不在对话区插入提示
    const shouldAutoReconnect = !manualDisconnect.value;
    if (shouldAutoReconnect) {
      const attempt = reconnectAttempts.value + 1;
      reconnectAttempts.value = attempt;
      const delay = Math.min(1000 * 2 ** (attempt - 1), 10000);
      if (reconnectTimer) clearTimeout(reconnectTimer);
      reconnectTimer = window.setTimeout(() => {
        connect();
      }, delay);
    }
  };
}

function disconnect() {
  manualDisconnect.value = true;
  socket.value?.close();
  socket.value = null;
  isConnecting.value = false;
  isConnected.value = false;
  pending.value = false;
  if (reconnectTimer) {
    clearTimeout(reconnectTimer);
    reconnectTimer = null;
  }
}

function clearChat() {
  messages.value = [];
  inputText.value = '';
  attachments.value = [];
}

function buildStopArray() {
  const raw = settings.stop.trim();
  if (!raw) return undefined;
  const parts = raw
    .split(',')
    .map(s => s.trim())
    .filter(Boolean);
  return parts.length ? parts : undefined;
}

function buildUserContent() {
  const text = inputText.value.trim();
  const imgMarkdown = attachments.value.map(a => `![${a.name}](${a.dataUrl})`).join('\n');
  const merged = [text, imgMarkdown].filter(Boolean).join('\n');
  return merged.trim();
}

function buildPayload() {
  const stopArr = buildStopArray();
  const seedVal = settings.seed.trim();
  const seed = seedVal ? Number(seedVal) : undefined;

  const payload: Record<string, any> = {
    messages: messages.value.map(m => ({ role: m.role, content: m.content })),
    temperature: settings.temperature,
    top_p: settings.topP,
    top_k: settings.topK,
    max_tokens: settings.maxTokens,
    frequency_penalty: settings.frequencyPenalty,
    presence_penalty: settings.presencePenalty,
    enable_thinking: settings.enableThinking
  };

  if (settings.model.trim()) payload.model = settings.model.trim();
  if (stopArr) payload.stop = stopArr;
  if (seed !== undefined && Number.isFinite(seed)) payload.seed = seed;

  return payload;
}

function canSend() {
  if (!isConnected.value) return false;
  if (pending.value) return false;
  if (!settings.apiKey.trim()) return false;
  const content = buildUserContent();
  return Boolean(content);
}

function send() {
  if (!canSend()) return;
  const ws = socket.value;
  if (!ws || ws.readyState !== WebSocket.OPEN) return;

  const content = buildUserContent();
  pushMessage('user', content);

  inputText.value = '';
  attachments.value = [];

  const payload = buildPayload();

  pending.value = true;
  responseState.value = 'streaming';
  currentMessageId.value = null;
  messageBuffer = '';
  const msg = {
    api_key: settings.apiKey.trim(),
    ...payload
  };
  ws.send(JSON.stringify(msg));
}

function handleKeydown(e: KeyboardEvent) {
  if ((e.ctrlKey || e.metaKey) && e.key === 'Enter') {
    e.preventDefault();
    send();
  }
}

async function handlePickImage(options: any) {
  const raw: File | undefined = options?.file?.file;
  const name: string = options?.file?.name || raw?.name || 'image';
  if (!raw) {
    options?.onFinish?.();
    return;
  }
  const reader = new FileReader();
  reader.readAsDataURL(raw);
  reader.onload = () => {
    const dataUrl = String(reader.result || '');
    if (!dataUrl) return;
    attachments.value.push({
      id: `${Date.now()}-${Math.random().toString(16).slice(2)}`,
      name,
      dataUrl
    });
    options?.onFinish?.();
  };
}

function removeAttachment(id: string) {
  attachments.value = attachments.value.filter(a => a.id !== id);
}

onMounted(() => {
  connect();
  if (messages.value.length === 0) {
    pushMessage('system', '你可以在右侧填写 APP_KEY，然后开始对话。Ctrl+Enter 发送。');
  }
});

onBeforeUnmount(() => {
  disconnect();
});
</script>

<template>
  <div class="h-full w-full p-2">
    <NGrid :x-gap="14" :y-gap="14" responsive="screen" item-responsive class="h-full">
      <NGi span="24 s:24 m:24">
        <NCard :bordered="false" class="relative h-full min-h-0 flex flex-col">
          <div class="mb-2 flex items-center justify-between gap-2">
            <div class="flex items-center gap-2">
              <NTag :type="isConnected ? 'success' : isConnecting ? 'warning' : 'error'" size="small">
                {{ isConnected ? '已连接' : isConnecting ? '连接中' : '未连接' }}
              </NTag>
              <NTag v-if="responseTag" :type="responseTag.type" size="small">
                {{ responseTag.text }}
              </NTag>
              <span class="break-all text-xs text-gray-500">{{ wsUrl }}</span>
            </div>
            <div class="flex items-center gap-2">
              <NButton size="small" :disabled="isConnected || isConnecting" @click="connect">重连</NButton>
              <NButton size="small" :disabled="!isConnected && !isConnecting" @click="disconnect">断开</NButton>
              <NButton size="small" @click="clearChat">清空</NButton>
            </div>
          </div>

          <div
            ref="listRef"
            class="max-h-[65vh] min-h-[200px] flex-1 overflow-auto border border-gray-200 rounded p-3 pb-[220px] dark:border-gray-800"
            @click="handleMessageClick"
          >
            <div class="space-y-3">
              <div
                v-for="m in messages"
                :key="m.id"
                class="flex"
                :class="m.role === 'user' ? 'justify-end' : 'justify-start'"
              >
                <div
                  class="max-w-[85%] rounded px-3 py-2"
                  :class="
                    m.role === 'user'
                      ? 'bg-primary text-white'
                      : m.role === 'system'
                        ? 'bg-gray-100 dark:bg-[#222] text-gray-700 dark:text-gray-200'
                        : 'bg-gray-50 dark:bg-[#1d1d1d] text-gray-800 dark:text-gray-100'
                  "
                >
                  <div class="mb-1 text-xs opacity-80">
                    {{ m.role }}
                    ·
                    {{ new Date(m.createdAt).toLocaleTimeString() }}
                  </div>
                  <div class="markdown-body" v-html="renderMarkdown(m.content)"></div>
                </div>
              </div>
            </div>
          </div>

          <div
            class="absolute bottom-0 left-0 right-0 border-t border-gray-200 bg-white p-2 dark:border-gray-800 dark:bg-[#141414]"
          >
            <div v-if="attachments.length" class="mb-2 flex flex-wrap gap-2">
              <div
                v-for="a in attachments"
                :key="a.id"
                class="relative h-20 w-20 overflow-hidden border border-gray-200 rounded dark:border-gray-800"
              >
                <img :src="a.dataUrl" :alt="a.name" class="h-full w-full object-cover" />
                <button
                  type="button"
                  class="absolute right-1 top-1 h-5 w-5 rounded bg-black/50 text-xs text-white"
                  @click="removeAttachment(a.id)"
                >
                  ×
                </button>
              </div>
            </div>

            <NInput
              v-model:value="inputText"
              type="textarea"
              :autosize="{ minRows: 3, maxRows: 8 }"
              placeholder="输入内容（Ctrl+Enter 发送，Shift+Enter 换行）"
              @keydown="handleKeydown"
            />
            <div class="mt-2 flex items-center justify-between gap-2">
              <div class="flex items-center gap-2">
                <NUpload accept="image/*" :show-file-list="false" :custom-request="handlePickImage">
                  <NButton size="small">上传图片</NButton>
                </NUpload>
              </div>
              <div class="flex items-center gap-2">
                <NButton size="small" secondary @click="showSettings = true">设置</NButton>
                <span v-if="sendDisabledReason" class="text-xs text-gray-500">{{ sendDisabledReason }}</span>
                <NButton type="primary" :loading="pending" :disabled="!canSend()" @click="send">发送</NButton>
              </div>
            </div>
          </div>
        </NCard>
      </NGi>
    </NGrid>
    <XModal v-model:show="showSettings" preset="card" title="参数设置" :width="720" :height="520" :mask-closable="true">
      <div class="h-full w-full p-3">
        <NForm label-placement="left" label-width="90">
          <NFormItem label="APP_KEY">
            <NInput
              v-model:value="settings.apiKey"
              type="password"
              show-password-on="click"
              placeholder="仅填 APP_KEY，不要加 Bearer"
            />
          </NFormItem>
          <NFormItem label="Model ID">
            <NInput v-model:value="settings.model" placeholder="可不填，后端有默认值" />
          </NFormItem>
          <NFormItem label="深度思考">
            <NSwitch v-model:value="settings.enableThinking" />
          </NFormItem>
          <NDivider />
          <div class="grid grid-cols-2 gap-3">
            <NFormItem label="温度">
              <NInputNumber v-model:value="settings.temperature" :min="0" :max="2" :step="0.1" class="w-full" />
            </NFormItem>
            <NFormItem label="多样性">
              <NInputNumber v-model:value="settings.topP" :min="0" :max="1" :step="0.01" class="w-full" />
            </NFormItem>
            <NFormItem label="top_k">
              <NInputNumber v-model:value="settings.topK" :min="1" :max="100" :step="1" class="w-full" />
            </NFormItem>
            <NFormItem label="最大tokens">
              <NInputNumber v-model:value="settings.maxTokens" :min="1" :max="16384" :step="128" class="w-full" />
            </NFormItem>
            <NFormItem label="重复惩罚">
              <NInputNumber v-model:value="settings.frequencyPenalty" :min="-2" :max="2" :step="0.1" class="w-full" />
            </NFormItem>
            <NFormItem label="存在惩罚">
              <NInputNumber v-model:value="settings.presencePenalty" :min="-2" :max="2" :step="0.1" class="w-full" />
            </NFormItem>
            <NFormItem label="随机种子">
              <NInput v-model:value="settings.seed" placeholder="可选，整数" />
            </NFormItem>
            <NFormItem label="停止序列">
              <NInput v-model:value="settings.stop" placeholder="可选，逗号分隔，例如：你好,再见" />
            </NFormItem>
          </div>
        </NForm>
      </div>
    </XModal>
  </div>
</template>

<style scoped>
:deep(.markdown-body) {
  word-break: break-word;
}

:deep(.markdown-body > :first-child) {
  margin-top: 0;
}

:deep(.markdown-body > :last-child) {
  margin-bottom: 0;
}

:deep(.markdown-body p) {
  margin: 0 0 8px;
}

:deep(.markdown-body pre) {
  margin: 8px 0;
  padding: 10px 12px;
  overflow: auto;
  border-radius: 8px;
  background: rgba(0, 0, 0, 0.06);
}

:deep(.markdown-body code) {
  font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, 'Liberation Mono', 'Courier New', monospace;
}

:deep(.markdown-body img) {
  display: block;
  max-width: 100%;
  height: auto;
  margin: 8px 0;
  border-radius: 8px;
}
</style>

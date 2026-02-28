<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue';
import { useMessage } from 'naive-ui';
import Clipboard from 'clipboard';
import * as monaco from 'monaco-editor';
import { VueMonacoEditor } from '@guolao/vue-monaco-editor';

const message = useMessage();

const inputText = ref(
  '{"name":"soybean-admin","features":{"format":true,"minify":true,"search":"key/value"},"items":[{"id":1,"title":"hello"},{"id":2,"title":"world"}]}'
);
const outputText = ref('');

const isValid = ref<boolean | null>(null);
const validateMessage = ref<string>('');

const unquoteKeys = ref(false);
const keyColor = ref('#4fc1ff');

const inputEditor = ref<monaco.editor.IStandaloneCodeEditor | null>(null);
const outputEditor = ref<monaco.editor.IStandaloneCodeEditor | null>(null);

let clipboard: Clipboard | null = null;

const clipboardDomRef = ref<HTMLElement | null>(null);

const outputLanguage = computed(() => (unquoteKeys.value ? 'javascript' : 'json'));
const editorTheme = computed(() => 'jsonview-dark');

const inputOptions: monaco.editor.IStandaloneEditorConstructionOptions = {
  automaticLayout: true,
  minimap: { enabled: true },
  fontSize: 14,
  tabSize: 2,
  insertSpaces: true,
  lineNumbers: 'on',
  folding: true,
  formatOnPaste: true,
  formatOnType: true
};

const outputOptions: monaco.editor.IStandaloneEditorConstructionOptions = {
  ...inputOptions,
  readOnly: true
};

function normalizeThemeColor(hex: string): string {
  const value = hex.trim().replace('#', '');
  if (value.length === 3) {
    return value
      .split('')
      .map(c => c + c)
      .join('');
  }
  if (value.length === 6) return value;
  return '4fc1ff';
}

function applyTheme() {
  const foreground = normalizeThemeColor(keyColor.value);

  monaco.editor.defineTheme('jsonview-dark', {
    base: 'vs-dark',
    inherit: true,
    rules: [
      { token: 'string.key.json', foreground },
      { token: 'identifier', foreground }
    ],
    colors: {}
  });
}

function positionToLineColumn(text: string, position: number) {
  let line = 1;
  let column = 1;
  const end = Math.min(position, text.length);
  for (let i = 0; i < end; i += 1) {
    if (text[i] === '\n') {
      line += 1;
      column = 1;
    } else {
      column += 1;
    }
  }
  return { line, column };
}

function getErrorPosition(error: unknown): number | null {
  const msg = error instanceof Error ? error.message : String(error);
  const match = msg.match(/position\s+(\d+)/i);
  if (!match) return null;
  const pos = Number(match[1]);
  return Number.isFinite(pos) ? pos : null;
}

function setInputMarkers(error: unknown | null) {
  const model = inputEditor.value?.getModel();
  if (!model) return;

  if (!error) {
    monaco.editor.setModelMarkers(model, 'jsonview', []);
    return;
  }

  const pos = getErrorPosition(error);
  if (pos === null) {
    monaco.editor.setModelMarkers(model, 'jsonview', [
      {
        severity: monaco.MarkerSeverity.Error,
        message: error instanceof Error ? error.message : String(error),
        startLineNumber: 1,
        startColumn: 1,
        endLineNumber: 1,
        endColumn: 1
      }
    ]);
    return;
  }

  const { line, column } = positionToLineColumn(inputText.value, pos);
  monaco.editor.setModelMarkers(model, 'jsonview', [
    {
      severity: monaco.MarkerSeverity.Error,
      message: error instanceof Error ? error.message : String(error),
      startLineNumber: line,
      startColumn: column,
      endLineNumber: line,
      endColumn: column + 1
    }
  ]);
}

function safeParseJson(text: string) {
  return JSON.parse(text) as unknown;
}

function stringifyJson(value: unknown, pretty: boolean) {
  return pretty ? JSON.stringify(value, null, 2) : JSON.stringify(value);
}

function unquoteJsonKeys(text: string) {
  return text.replace(/"([A-Za-z_$][0-9A-Za-z_$]*)"\s*:/g, '$1:');
}

function runValidate() {
  try {
    safeParseJson(inputText.value);
    isValid.value = true;
    validateMessage.value = 'JSON 格式正确';
    setInputMarkers(null);
    message.success('JSON 格式正确');
    return true;
  } catch (e) {
    isValid.value = false;
    validateMessage.value = e instanceof Error ? e.message : String(e);
    setInputMarkers(e);
    message.error('JSON 格式不正确');
    return false;
  }
}

function applyOutputText(text: string) {
  outputText.value = unquoteKeys.value ? unquoteJsonKeys(text) : text;
}

function handleFormat() {
  try {
    const obj = safeParseJson(inputText.value);
    isValid.value = true;
    validateMessage.value = 'JSON 格式正确';
    setInputMarkers(null);
    applyOutputText(stringifyJson(obj, true));
  } catch (e) {
    isValid.value = false;
    validateMessage.value = e instanceof Error ? e.message : String(e);
    setInputMarkers(e);
    outputText.value = '';
  }
}

function handleMinify() {
  try {
    const obj = safeParseJson(inputText.value);
    isValid.value = true;
    validateMessage.value = 'JSON 格式正确';
    setInputMarkers(null);
    applyOutputText(stringifyJson(obj, false));
  } catch (e) {
    isValid.value = false;
    validateMessage.value = e instanceof Error ? e.message : String(e);
    setInputMarkers(e);
    outputText.value = '';
  }
}

function foldAll() {
  outputEditor.value?.getAction('editor.foldAll')?.run();
}

function unfoldAll() {
  outputEditor.value?.getAction('editor.unfoldAll')?.run();
}

function handleInputMount(editor: monaco.editor.IStandaloneCodeEditor) {
  inputEditor.value = editor;
  applyTheme();
  handleFormat();
}

function handleOutputMount(editor: monaco.editor.IStandaloneCodeEditor) {
  outputEditor.value = editor;
  applyTheme();
}

function handleFind() {
  outputEditor.value?.getAction('actions.find')?.run();
}

function initClipboard() {
  if (!clipboardDomRef.value) return;
  clipboard = new Clipboard(clipboardDomRef.value);
  clipboard.on('success', () => {
    message.success('复制成功');
  });
  clipboard.on('error', () => {
    message.error('复制失败');
  });
}

onMounted(() => {
  initClipboard();
});

onBeforeUnmount(() => {
  outputEditor.value?.dispose();
  inputEditor.value?.dispose();
  clipboard?.destroy();
  clipboard = null;
});

watch(keyColor, () => {
  applyTheme();
});

watch(unquoteKeys, () => {
  if (!outputText.value) return;
  const current = outputText.value;
  if (unquoteKeys.value) {
    outputText.value = unquoteJsonKeys(current);
  } else {
    try {
      const parsed = safeParseJson(inputText.value);
      outputText.value = stringifyJson(parsed, true);
    } catch {
      outputText.value = current;
    }
  }
});
</script>

<template>
  <div class="h-full w-full flex flex-col gap-3 p-3">
    <div class="flex flex-wrap items-center gap-2">
      <NButton type="primary" @click="handleFormat">格式化</NButton>
      <NButton @click="handleMinify">压缩一行</NButton>
      <NButton @click="runValidate">检测格式</NButton>
      <div ref="clipboardDomRef" data-clipboard-target="#jsonViewCopyTarget">
        <NButton :disabled="!outputText" type="success">复制结果</NButton>
      </div>

      <div class="flex items-center gap-2">
        <span class="text-12px text-gray-500">去除 key 引号</span>
        <NSwitch v-model:value="unquoteKeys" />
      </div>

      <div class="flex items-center gap-2">
        <span class="text-12px text-gray-500">key 颜色</span>
        <NColorPicker v-model:value="keyColor" :show-alpha="false" class="w-120px" />
      </div>

      <div class="flex items-center gap-2">
        <NButton size="small" @click="handleFind">搜索</NButton>
      </div>

      <div class="flex items-center gap-2">
        <NButton size="small" @click="unfoldAll">展开</NButton>
        <NButton size="small" @click="foldAll">折叠</NButton>
      </div>

      <textarea id="jsonViewCopyTarget" v-model="outputText" class="absolute opacity-0 -z-1" />
    </div>

    <NAlert v-if="isValid !== null" :type="isValid ? 'success' : 'error'" :show-icon="false">
      {{ validateMessage }}
    </NAlert>

    <div class="grid grid-cols-2 min-h-520px flex-1 gap-3">
      <div class="flex flex-col overflow-hidden border rounded">
        <div class="bg-#f7f7f7 px-3 py-2 text-12px text-gray-500 dark:bg-#1f1f1f">输入（JSON 字符串）</div>
        <div class="flex-1 overflow-hidden">
          <VueMonacoEditor
            v-model:value="inputText"
            :theme="editorTheme"
            language="json"
            :options="inputOptions"
            @mount="handleInputMount"
          />
        </div>
      </div>

      <div class="flex flex-col overflow-hidden border rounded">
        <div class="bg-#f7f7f7 px-3 py-2 text-12px text-gray-500 dark:bg-#1f1f1f">输出（JSON 视图）</div>
        <div class="flex-1 overflow-hidden">
          <VueMonacoEditor
            v-model:value="outputText"
            :theme="editorTheme"
            :language="outputLanguage"
            :options="outputOptions"
            @mount="handleOutputMount"
          />
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
:deep(.monaco-editor) {
  height: 100%;
}
</style>

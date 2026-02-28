<script setup lang="ts">
import { computed, ref, watch } from 'vue';
import { NButton, NModal } from 'naive-ui';
import { VueMonacoEditor } from '@guolao/vue-monaco-editor';

interface Props {
  show: boolean;
  filename: string;
  content: string;
  loading?: boolean;
}

interface Emits {
  (e: 'update:show', visible: boolean): void;
  (e: 'save', content: string): void;
  (e: 'close'): void;
}

const props = defineProps<Props>();
const emit = defineEmits<Emits>();

const visible = computed({
  get: () => props.show,
  set: val => emit('update:show', val)
});

const code = ref('');

watch(
  () => props.content,
  newContent => {
    code.value = newContent || '';
  },
  { immediate: true }
);

const extensionLanguageMap: Record<string, string> = {
  js: 'typescript',
  jsx: 'typescript',
  ts: 'typescript',
  tsx: 'typescript',
  vue: 'html',
  json: 'json',
  css: 'css',
  scss: 'scss',
  html: 'html',
  md: 'markdown',
  rs: 'rust',
  py: 'python',
  go: 'go',
  java: 'java',
  c: 'cpp',
  cpp: 'cpp',
  h: 'cpp',
  sh: 'shell',
  bash: 'shell',
  yaml: 'yaml',
  yml: 'yaml',
  xml: 'xml',
  sql: 'sql'
};

const language = computed(() => {
  const ext = props.filename.split('.').pop()?.toLowerCase() ?? '';
  return extensionLanguageMap[ext] ?? 'plaintext';
});

const editorOptions = {
  automaticLayout: true,
  formatOnType: true,
  formatOnPaste: true,
  minimap: {
    enabled: true
  },
  fontSize: 14,
  tabSize: 2
};

const handleMount = (_editor: any) => {};

const handleClose = () => {
  visible.value = false;
  emit('close');
};

const handleSave = () => {
  emit('save', code.value);
};
</script>

<template>
  <NModal
    v-model:show="visible"
    preset="card"
    :title="`编辑文件: ${filename}`"
    class="h-80vh w-80vw"
    :mask-closable="false"
    :on-close="handleClose"
  >
    <div class="h-full flex flex-col gap-4">
      <div class="flex-1 overflow-hidden border rounded">
        <VueMonacoEditor
          v-model:value="code"
          theme="vs-dark"
          :language="language"
          :options="editorOptions"
          @mount="handleMount"
        />
      </div>
      <div class="flex justify-end gap-2">
        <NButton @click="handleClose">取消</NButton>
        <NButton type="primary" :loading="loading" @click="handleSave">保存</NButton>
      </div>
    </div>
  </NModal>
</template>

<style scoped>
:deep(.monaco-editor) {
  height: 100%;
}
</style>

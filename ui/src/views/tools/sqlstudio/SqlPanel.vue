<script setup lang="ts">
import { ref } from 'vue';
import { Play } from '@vicons/ionicons5';
import { MagicWand } from '@vicons/carbon';
import { VueMonacoEditor } from '@guolao/vue-monaco-editor';
import type { editor as MonacoEditor } from 'monaco-editor';

// 编辑器完整配置（带中文注释）
const editorOptions: MonacoEditor.IStandaloneEditorConstructionOptions = {
  // —— 基础与布局 ——
  automaticLayout: true, // 自动布局（容器大小变化时自适应）
  readOnly: false, // 是否只读
  ariaLabel: 'SQL Editor', // 可访问性标签
  fixedOverflowWidgets: true, // 将悬浮窗口固定在编辑器内部

  // —— 视图与渲染 ——
  lineNumbers: 'on', // 行号：on/off/relative/interval
  lineNumbersMinChars: 2, // 行号最小字符宽度
  glyphMargin: true, // 符号边距（断点、折叠图标所在列）
  renderLineHighlight: 'all', // 高亮当前行（line/gutter/all/none）
  renderWhitespace: 'boundary', // 显示空白字符（none/boundary/selection/trailing/all）
  renderControlCharacters: false, // 显示控制字符
  rulers: [], // 标尺列（传入列号数组）
  minimap: {
    enabled: false, // 是否显示小地图
    renderCharacters: true, // 小地图是否渲染字符
    maxColumn: 120 // 小地图最大列数
  },
  smoothScrolling: true, // 平滑滚动
  scrollBeyondLastLine: true, // 允许滚动超过最后一行

  // —— 折叠与缩进 ——
  folding: true, // 启用代码折叠
  foldingStrategy: 'auto', // 折叠策略：auto/indentation
  showFoldingControls: 'always', // 折叠控件显示：always/mouseover
  tabSize: 2, // Tab 宽度
  insertSpaces: true, // 用空格代替 Tab
  detectIndentation: true, // 自动检测缩进
  autoIndent: 'advanced', // 自动缩进（none/keep/brackets/advanced/full）

  // —— 光标与选择 ——
  cursorStyle: 'line', // 光标样式（line/block/underline/…）
  cursorBlinking: 'blink', // 光标闪烁样式
  cursorSmoothCaretAnimation: 'on', // 光标平滑动画（on/off/explicit）
  cursorSurroundingLines: 3, // 光标上下保留行
  cursorSurroundingLinesStyle: 'default', // 上下保留行策略（default/all）
  selectionHighlight: true, // 高亮与所选内容相同的内容
  occurrencesHighlight: 'singleFile', // 高亮光标处相同内容（off/singleFile/multiFile）

  // —— 包裹与换行 ——
  wordWrap: 'on', // 自动换行（off/on/wordWrapColumn/bounded）
  wordWrapColumn: 120, // 换行列
  wrappingIndent: 'same', // 换行缩进（none/same/indent/deepIndent）

  // —— 建议与提示 ——
  links: true, // 使链接可点击
  codeLens: false, // 代码透镜（上方小信息）
  quickSuggestions: { other: true, comments: false, strings: false }, // 快速建议源
  quickSuggestionsDelay: 200, // 快速建议延迟
  parameterHints: { enabled: true, cycle: true }, // 参数提示

  // —— 格式化 ——
  formatOnPaste: true, // 粘贴时格式化
  formatOnType: true, // 输入时格式化

  // —— 辅助功能 ——
  accessibilitySupport: 'auto' // 无障碍支持（auto/on/off）
};

const code = ref('');
const handleMount = (_editor: any) => {};
</script>

<template>
  <div class="bs-shadow-md h-full w-full flex flex-col gap-1 overflow-hidden" :style="{}">
    <div class="shrink-0">
      <NSpace :size="5">
        <NButtonGroup size="small">
          <NButton ghost>
            <template #icon>
              <NIcon color="green">
                <Play />
              </NIcon>
            </template>
          </NButton>
          <NButton>
            <template #icon>
              <NIcon color="blue">
                <MagicWand />
              </NIcon>
            </template>
          </NButton>
        </NButtonGroup>
      </NSpace>
    </div>
    <div class="flex-1 overflow-hidden border">
      <VueMonacoEditor
        v-model:value="code"
        theme="vs-light"
        language="sql"
        :options="editorOptions"
        class="h-full w-full"
        @mount="handleMount"
      />
    </div>
  </div>
</template>

<style scoped></style>

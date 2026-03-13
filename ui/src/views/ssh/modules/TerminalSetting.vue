<script setup lang="ts">
import { reactive, ref, watch } from 'vue';
import { NCard, NColorPicker, NForm, NFormItem, NIcon, NInputNumber, NSelect, NSwitch } from 'naive-ui';
import { CloseOutline } from '@vicons/ionicons5';

const props = defineProps<{
  initialSettings?: {
    fontSize: number;
    background: string;
    foreground: string;
    cursor: string;
    cursorStyle: 'block' | 'underline' | 'bar';
    cursorBlink: boolean;
  };
}>();

const emit = defineEmits<{
  (e: 'updateSettings', settings: any): void;
  (e: 'close'): void;
}>();

const settings = reactive({
  fontSize: props.initialSettings?.fontSize ?? 14,
  background: props.initialSettings?.background ?? '#1e1e1e',
  foreground: props.initialSettings?.foreground ?? '#d4d4d4',
  cursor: props.initialSettings?.cursor ?? '#ffffff',
  cursorStyle: props.initialSettings?.cursorStyle ?? 'block',
  cursorBlink: props.initialSettings?.cursorBlink ?? true
});

const cursorOptions = [
  { label: 'Block (█)', value: 'block' },
  { label: 'Underline (_)', value: 'underline' },
  { label: 'Bar (|)', value: 'bar' }
];

watch(
  settings,
  newVal => {
    emit('updateSettings', {
      fontSize: newVal.fontSize,
      cursorStyle: newVal.cursorStyle,
      cursorBlink: newVal.cursorBlink,
      theme: {
        background: newVal.background,
        foreground: newVal.foreground,
        cursor: newVal.cursor
      }
    });
  },
  { deep: true }
);
</script>

<template>
  <div class="pointer-events-auto absolute right-0 top-0 z-10">
    <NCard
      size="small"
      class="w-62 !rounded-none !border-none !bg-opacity-80 !backdrop-blur-sm"
      :content-style="{ padding: '12px' }"
    >
      <div class="absolute right-1 top-1 z-10 cursor-pointer" @click="emit('close')">
        <NIcon size="16" class="text-gray-400 hover:text-white"><CloseOutline /></NIcon>
      </div>

      <div class="mt-2 space-y-4">
        <NForm size="small" label-placement="left" label-width="70" class="setting-form">
          <NFormItem label="字体大小">
            <NInputNumber v-model:value="settings.fontSize" :min="10" :max="32" size="small" class="w-full" />
          </NFormItem>
          <NFormItem label="背景颜色">
            <NColorPicker v-model:value="settings.background" :show-alpha="false" size="small" />
          </NFormItem>
          <NFormItem label="字体颜色">
            <NColorPicker v-model:value="settings.foreground" :show-alpha="false" size="small" />
          </NFormItem>
          <NFormItem label="光标颜色">
            <NColorPicker v-model:value="settings.cursor" :show-alpha="false" size="small" />
          </NFormItem>
          <NFormItem label="光标样式">
            <NSelect v-model:value="settings.cursorStyle" :options="cursorOptions" size="small" />
          </NFormItem>
          <NFormItem label="光标闪烁">
            <NSwitch v-model:value="settings.cursorBlink" size="small" />
          </NFormItem>
        </NForm>
      </div>
    </NCard>
  </div>
</template>

<style scoped>
:deep(.n-card) {
  background-color: rgba(0, 0, 0, 0.6) !important;
  color: white;
}
:deep(.n-form-item-label) {
  color: #e5e7eb !important;
}
:deep(.n-form-item) {
  --n-feedback-height: 14px !important;
}
</style>

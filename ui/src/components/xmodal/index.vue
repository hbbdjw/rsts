<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from 'vue';
import { NButton, NCard, NIcon, NModal } from 'naive-ui';
import { CloseCircleOutline, ContractSharp, ExpandSharp } from '@vicons/ionicons5';
let globalZIndex = 2000;

defineOptions({ name: 'XModal' });

interface Props {
  show?: boolean;
  title?: string;
  width?: string | number;
  height?: string | number;
  minWidth?: number;
  minHeight?: number;
  x?: number;
  y?: number;
  preset?: 'card' | 'dialog' | 'confirm';
  to?: string | HTMLElement;
  mask?: boolean;
  showMask?: boolean;
  inline?: boolean;
  showHeader?: boolean;
  draggable?: boolean;
  resizable?: boolean;
  shadow?: boolean;
  zIndex?: number;
}

const props = withDefaults(defineProps<Props>(), {
  show: false,
  title: 'Modal',
  width: 600,
  height: 400,
  minWidth: 200,
  minHeight: 150,
  x: 100,
  y: 100,
  preset: 'card',
  to: 'body',
  mask: false,
  showMask: undefined,
  inline: false,
  showHeader: true,
  draggable: true,
  resizable: true,
  shadow: true,
  zIndex: undefined
});

const showMask = computed(() => (props.showMask ?? props.mask) === true);
const isInline = computed(() => props.inline === true);
const showHeader = computed(() => props.showHeader !== false);
const allowDrag = computed(() => props.draggable !== false);
const allowResize = computed(() => props.resizable !== false);
const showShadow = computed(() => props.shadow !== false);

const emit = defineEmits<{
  (e: 'update:show', visible: boolean): void;
  (e: 'close'): void;
  (e: 'update:x', x: number): void;
  (e: 'update:y', y: number): void;
  (e: 'update:width', width: number): void;
  (e: 'update:height', height: number): void;
  (e: 'focus', zIndex: number): void;
}>();

const visible = computed({
  get: () => props.show,
  set: val => emit('update:show', val)
});

const currentX = ref(props.x);
const currentY = ref(props.y);
const currentW = ref(typeof props.width === 'number' ? props.width : Number.parseInt(props.width as string, 10) || 600);
const currentH = ref(
  typeof props.height === 'number' ? props.height : Number.parseInt(props.height as string, 10) || 400
);
const zIndex = ref(props.zIndex ?? globalZIndex);
const isMaximized = ref(false);
const restoreState = { x: 0, y: 0, w: 0, h: 0 };

const bringToFront = () => {
  if (isInline.value) return;
  globalZIndex = Math.max(globalZIndex + 1, props.zIndex || 0);
  zIndex.value = globalZIndex;
  emit('focus', zIndex.value);
};

const headerRef = ref<HTMLElement | null>(null);
const isDragging = ref(false);
const dragStart = { x: 0, y: 0 };
const initialPos = { x: 0, y: 0 };

const clampValue = (value: number, min: number, max: number) => Math.min(max, Math.max(min, value));

const clampToViewport = () => {
  const headerHeight = headerRef.value?.getBoundingClientRect().height ?? 40;
  const maxX = Math.max(0, window.innerWidth - currentW.value);
  const maxY = Math.max(0, window.innerHeight - headerHeight);
  currentX.value = clampValue(currentX.value, 0, maxX);
  currentY.value = clampValue(currentY.value, 0, maxY);
};

const handleWindowResize = () => {
  if (!isMaximized.value) return;
  currentX.value = 0;
  currentY.value = 0;
  currentW.value = window.innerWidth;
  currentH.value = window.innerHeight;
};

const handleMouseMove = (e: MouseEvent) => {
  if (!isDragging.value) return;
  const dx = e.clientX - dragStart.x;
  const dy = e.clientY - dragStart.y;
  currentX.value = initialPos.x + dx;
  currentY.value = initialPos.y + dy;
};

const handleMouseUp = () => {
  isDragging.value = false;
  window.removeEventListener('mousemove', handleMouseMove);
  window.removeEventListener('mouseup', handleMouseUp);
  clampToViewport();
  emit('update:x', currentX.value);
  emit('update:y', currentY.value);
  emit('focus', zIndex.value);
};

const handleMouseDown = (e: MouseEvent) => {
  if (!allowDrag.value) return;
  if (isMaximized.value) return;
  if (e.target !== headerRef.value && !headerRef.value?.contains(e.target as Node)) return;
  if ((e.target as HTMLElement).closest('.n-button') || (e.target as HTMLElement).closest('.n-icon')) return;

  isDragging.value = true;
  dragStart.x = e.clientX;
  dragStart.y = e.clientY;
  initialPos.x = currentX.value;
  initialPos.y = currentY.value;

  bringToFront();

  window.addEventListener('mousemove', handleMouseMove);
  window.addEventListener('mouseup', handleMouseUp);
};

const isResizing = ref(false);
const resizeStart = { x: 0, y: 0 };
const initialSize = { w: 0, h: 0 };
const initialResizePos = { x: 0, y: 0 };
let activeHandle = '';

const handleResizeMove = (e: MouseEvent) => {
  if (!isResizing.value) return;
  const dx = e.clientX - resizeStart.x;
  const dy = e.clientY - resizeStart.y;

  if (activeHandle.includes('e')) {
    currentW.value = Math.max(props.minWidth, initialSize.w + dx);
  }
  if (activeHandle.includes('s')) {
    currentH.value = Math.max(props.minHeight, initialSize.h + dy);
  }
  if (activeHandle.includes('w')) {
    const newW = Math.max(props.minWidth, initialSize.w - dx);
    currentW.value = newW;
    currentX.value = initialResizePos.x + (initialSize.w - newW);
  }
  if (activeHandle.includes('n')) {
    const newH = Math.max(props.minHeight, initialSize.h - dy);
    currentH.value = newH;
    currentY.value = initialResizePos.y + (initialSize.h - newH);
  }
};

const handleResizeUp = () => {
  isResizing.value = false;
  window.removeEventListener('mousemove', handleResizeMove);
  window.removeEventListener('mouseup', handleResizeUp);
  emit('update:width', currentW.value);
  emit('update:height', currentH.value);
  emit('focus', zIndex.value);
};

const startResize = (handle: string, e: MouseEvent) => {
  if (!allowResize.value) return;
  e.preventDefault();
  e.stopPropagation();
  isResizing.value = true;
  activeHandle = handle;
  resizeStart.x = e.clientX;
  resizeStart.y = e.clientY;
  initialSize.w = currentW.value;
  initialSize.h = currentH.value;
  initialResizePos.x = currentX.value;
  initialResizePos.y = currentY.value;

  bringToFront();

  window.addEventListener('mousemove', handleResizeMove);
  window.addEventListener('mouseup', handleResizeUp);
};
const handleClose = () => {
  visible.value = false;
  emit('close');
};

const handleToggleMaximize = () => {
  bringToFront();
  if (!isMaximized.value) {
    restoreState.x = currentX.value;
    restoreState.y = currentY.value;
    restoreState.w = currentW.value;
    restoreState.h = currentH.value;
    currentX.value = 0;
    currentY.value = 0;
    currentW.value = window.innerWidth;
    currentH.value = window.innerHeight;
    isMaximized.value = true;
    return;
  }
  currentX.value = restoreState.x;
  currentY.value = restoreState.y;
  currentW.value = restoreState.w;
  currentH.value = restoreState.h;
  isMaximized.value = false;
  clampToViewport();
};

onMounted(() => {
  bringToFront();
  window.addEventListener('resize', handleWindowResize);
});

onUnmounted(() => {
  window.removeEventListener('mousemove', handleMouseMove);
  window.removeEventListener('mouseup', handleMouseUp);
  window.removeEventListener('mousemove', handleResizeMove);
  window.removeEventListener('mouseup', handleResizeUp);
  window.removeEventListener('resize', handleWindowResize);
});

watch(
  () => [props.x, props.y, props.width, props.height] as const,
  ([x, y, width, height]) => {
    if (isDragging.value || isResizing.value || isMaximized.value) return;
    if (typeof x === 'number') currentX.value = x;
    if (typeof y === 'number') currentY.value = y;
    if (typeof width === 'number') currentW.value = width;
    else if (typeof width === 'string') currentW.value = Number.parseInt(width, 10) || currentW.value;
    if (typeof height === 'number') currentH.value = height;
    else if (typeof height === 'string') currentH.value = Number.parseInt(height, 10) || currentH.value;
  }
);

watch(
  () => props.show,
  val => {
    if (val) bringToFront();
  }
);

const wrapperStyle = computed(() => {
  if (isInline.value) {
    return {
      left: '0px',
      top: '0px',
      width: '100%',
      height: '100%',
      zIndex: 'auto'
    };
  }
  return {
    left: `${currentX.value}px`,
    top: `${currentY.value}px`,
    width: `${currentW.value}px`,
    height: `${currentH.value}px`,
    zIndex: zIndex.value
  };
});

const wrapperClass = computed(() => ({
  'x-modal-inline': isInline.value,
  'x-modal-shadow': showShadow.value
}));
</script>

<template>
  <NModal
    v-if="showMask"
    v-model:show="visible"
    :mask="showMask"
    :show-mask="showMask"
    :mask-closable="showMask"
    :to="to"
    :preset="preset"
    :trap-focus="false"
    :block-scroll="false"
    :z-index="zIndex"
    class="x-modal-container"
  >
    <div
      class="x-modal-wrapper shadow-xs pointer-events-auto absolute flex flex-col rounded bg-white dark:bg-[#18181c]"
      :class="{ 'x-modal-maximized': isMaximized }"
      :style="{
        left: `${currentX}px`,
        top: `${currentY}px`,
        width: `${currentW}px`,
        height: `${currentH}px`,
        zIndex
      }"
      @mousedown="bringToFront"
    >
      <NCard class="x-modal-card h-full w-full flex flex-col" :bordered="true">
        <template #header>
          <div ref="headerRef" class="x-modal-header h-full w-full flex items-center" @mousedown="handleMouseDown">
            <span>{{ title }}</span>
          </div>
        </template>

        <template #header-extra>
          <div class="flex items-center gap-2">
            <slot name="header-extra" />
            <NButton text size="small" @click="handleToggleMaximize">
              <template #icon>
                <NIcon :size="18">
                  <ContractSharp v-if="isMaximized" />
                  <ExpandSharp v-else />
                </NIcon>
              </template>
            </NButton>
            <NButton text size="small" @click="handleClose">
              <template #icon>
                <NIcon :size="18"><CloseCircleOutline /></NIcon>
              </template>
            </NButton>
          </div>
        </template>

        <div class="relative flex-1 overflow-auto">
          <slot />
        </div>

        <template v-if="$slots.footer" #footer>
          <slot name="footer" />
        </template>
        <template v-if="$slots.action" #action>
          <slot name="action" />
        </template>
      </NCard>

      <div class="resize-handle n" @mousedown="startResize('n', $event)" />
      <div class="resize-handle s" @mousedown="startResize('s', $event)" />
      <div class="resize-handle e" @mousedown="startResize('e', $event)" />
      <div class="resize-handle w" @mousedown="startResize('w', $event)" />
      <div class="resize-handle ne" @mousedown="startResize('ne', $event)" />
      <div class="resize-handle nw" @mousedown="startResize('nw', $event)" />
      <div class="resize-handle se" @mousedown="startResize('se', $event)" />
      <div class="resize-handle sw" @mousedown="startResize('sw', $event)" />
    </div>
  </NModal>
  <Teleport v-else :to="to" :disabled="isInline || !to">
    <div v-show="visible">
      <div
        class="x-modal-wrapper pointer-events-auto flex flex-col rounded bg-white dark:bg-[#18181c]"
        :class="[wrapperClass, { 'x-modal-maximized': isMaximized }]"
        :style="wrapperStyle"
        @mousedown="bringToFront"
      >
        <NCard class="x-modal-card h-full w-full flex flex-col" :bordered="showHeader">
          <template v-if="showHeader" #header>
            <div ref="headerRef" class="x-modal-header h-full w-full flex items-center" @mousedown="handleMouseDown">
              <span>{{ title }}</span>
            </div>
          </template>

          <template v-if="showHeader" #header-extra>
            <div class="flex items-center gap-2">
              <slot name="header-extra" />
              <NButton text size="small" @click="handleToggleMaximize">
                <template #icon>
                  <NIcon :size="18">
                    <ContractSharp v-if="isMaximized" />
                    <ExpandSharp v-else />
                  </NIcon>
                </template>
              </NButton>
              <NButton text size="small" @click="handleClose">
                <template #icon>
                  <NIcon :size="18"><CloseCircleOutline /></NIcon>
                </template>
              </NButton>
            </div>
          </template>

          <div class="relative flex-1 overflow-auto">
            <slot />
          </div>

          <template v-if="$slots.footer" #footer>
            <slot name="footer" />
          </template>
          <template v-if="$slots.action" #action>
            <slot name="action" />
          </template>
        </NCard>

        <div v-if="!isInline && !isMaximized && allowResize">
          <div class="resize-handle n" @mousedown="startResize('n', $event)" />
          <div class="resize-handle s" @mousedown="startResize('s', $event)" />
          <div class="resize-handle e" @mousedown="startResize('e', $event)" />
          <div class="resize-handle w" @mousedown="startResize('w', $event)" />
          <div class="resize-handle ne" @mousedown="startResize('ne', $event)" />
          <div class="resize-handle nw" @mousedown="startResize('nw', $event)" />
          <div class="resize-handle se" @mousedown="startResize('se', $event)" />
          <div class="resize-handle sw" @mousedown="startResize('sw', $event)" />
        </div>
      </div>
    </div>
  </Teleport>
</template>

<style scoped>
.x-modal-wrapper {
  position: fixed;
}

.x-modal-shadow {
  box-shadow: 0 4px 16px rgba(0, 0, 0, 0.15);
}

.x-modal-inline {
  position: absolute;
  box-shadow: none;
}

.x-modal-header {
  cursor: move;
  user-select: none;
}

.x-modal-maximized .resize-handle {
  display: none;
}

:deep(.x-modal-card .n-card__content) {
  padding: 0;
  flex: 1;
  overflow: hidden;
  display: flex;
  flex-direction: column;
}
:deep(.x-modal-card .n-card-header) {
  padding-top: 5px;
  padding-bottom: 5px;
  padding-left: 8px;
  padding-right: 8px;
  border-bottom: 1px solid #e5e5e5;
}
:deep(.x-modal-card .n-card-footer) {
  padding: 8px 8px;
}
:deep(.x-modal-card .x-modal-wrapper) {
}
.resize-handle {
  position: absolute;
  z-index: 100;
}

.resize-handle.n {
  top: -5px;
  left: 5px;
  right: 5px;
  height: 10px;
  cursor: ns-resize;
}
.resize-handle.s {
  bottom: -5px;
  left: 5px;
  right: 5px;
  height: 10px;
  cursor: ns-resize;
}
.resize-handle.e {
  right: -5px;
  top: 5px;
  bottom: 5px;
  width: 10px;
  cursor: ew-resize;
}
.resize-handle.w {
  left: -5px;
  top: 5px;
  bottom: 5px;
  width: 10px;
  cursor: ew-resize;
}

.resize-handle.ne {
  top: -5px;
  right: -5px;
  width: 15px;
  height: 15px;
  cursor: ne-resize;
}
.resize-handle.nw {
  top: -5px;
  left: -5px;
  width: 15px;
  height: 15px;
  cursor: nw-resize;
}
.resize-handle.se {
  bottom: -5px;
  right: -5px;
  width: 15px;
  height: 15px;
  cursor: se-resize;
}
.resize-handle.sw {
  bottom: -5px;
  left: -5px;
  width: 15px;
  height: 15px;
  cursor: sw-resize;
}
</style>

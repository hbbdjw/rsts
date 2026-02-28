<script setup lang="ts">
import { ref } from 'vue';
import Test1 from './modules/Test1.vue';
import JsonView from './modules/JsonView.vue';

const showModal1 = ref(false);
const showModal2 = ref(false);

const vRipple = {
  mounted(el: HTMLElement) {
    const style = window.getComputedStyle(el);
    if (style.position === 'static') {
      el.style.position = 'relative';
    }
    el.style.overflow = 'hidden';
    el.addEventListener('click', e => {
      const circle = document.createElement('span');
      const diameter = Math.max(el.clientWidth, el.clientHeight);
      const radius = diameter / 2;
      const rect = el.getBoundingClientRect();

      // Calculate position relative to the padding box (which is the containing block for absolute children)
      // We need to subtract the border width because rect.left is the border edge, but absolute positioning starts at padding edge
      const borderLeft = Number.parseFloat(style.borderLeftWidth) || 0;
      const borderTop = Number.parseFloat(style.borderTopWidth) || 0;

      circle.style.width = `${diameter}px`;
      circle.style.height = `${diameter}px`;
      circle.style.left = `${e.clientX - rect.left - borderLeft - radius}px`;
      circle.style.top = `${e.clientY - rect.top - borderTop - radius}px`;
      circle.classList.add('ripple-effect');

      const existingRipple = el.getElementsByClassName('ripple-effect')[0];
      if (existingRipple) {
        existingRipple.remove();
      }

      el.appendChild(circle);

      setTimeout(() => {
        circle.remove();
      }, 600);
    });
  }
};
const handleClick1 = () => {
  showModal1.value = true;
};
const handleClick2 = () => {
  showModal2.value = true;
};
</script>

<template>
  <div class="h-full w-full p-3">
    <NFlex :size="10">
      <NFloatButton v-ripple shape="square" @click="handleClick1">
        <!-- <div class="flex items-center justify-center w-full h-full"> -->
        <!--
 <n-icon>
                        <Apps />
                    </n-icon> 
-->
        <!-- </div> -->
        <template #description>Sobel边缘检测</template>
      </NFloatButton>
      <NFloatButton v-ripple shape="square" @click="handleClick2">
        <!-- <div class="flex items-center justify-center w-full h-full"> -->
        <!--
 <n-icon>
                        <Apps />
                    </n-icon> 
-->
        <!-- </div> -->
        <template #description>JsonView</template>
      </NFloatButton>
    </NFlex>
    <NModal v-model:show="showModal1" title="Sobel边缘检测" preset="card" draggable class="w-800px">
      <Test1 />
    </NModal>
    <NModal v-model:show="showModal2" title="JsonView" preset="card" draggable class="w-800px">
      <JsonView />
    </NModal>
  </div>
</template>

<style scoped>
:deep(.n-float-button) {
  position: relative !important;
  inset: auto !important;
  width: 100px !important;
  height: 100px !important;
}

:deep(.n-float-button__description) {
  white-space: pre-wrap !important;
  line-height: 1.2;
  font-size: 14px;
  display: flex;
  align-items: center;
  justify-content: center;
  text-align: center;
  padding: 4px;
  width: 100%;
  height: 100%;
}

:deep(.ripple-effect) {
  position: absolute;
  border-radius: 50%;
  transform: scale(0);
  animation: ripple 0.6s linear;
  background-color: rgba(0, 0, 0, 0.3);
  pointer-events: none;
  z-index: 9999;
}

@keyframes ripple {
  to {
    transform: scale(4);
    opacity: 0;
  }
}
</style>

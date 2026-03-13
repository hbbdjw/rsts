<script setup lang="ts">
import MenuPanel from './MenuPanel.vue';
import SqlPanel from './SqlPanel.vue';
import ResultPanel from './ResultPanel.vue';
</script>

<template>
  <div class="h-full w-full overflow-hidden">
    <NSplit direction="horizontal" :min="0.1" :max="0.9" :default-size="0.1" :resize-trigger-size="2">
      <template #resize-trigger>
        <div class="h-full w-full flex items-center justify-center">
          <div class="pointer-events-none h-full w-px bg-gray-300 dark:bg-gray-600"></div>
        </div>
      </template>
      <template #1>
        <MenuPanel />
      </template>
      <template #2>
        <NSplit direction="vertical" :min="0.1" :max="0.9" :default-size="0.5" :resize-trigger-size="2">
          <template #resize-trigger>
            <div class="h-full w-full flex items-center justify-center">
              <div class="pointer-events-none h-px w-full bg-gray-300 dark:bg-gray-600"></div>
            </div>
          </template>
          <template #1>
            <div class="h-full w-full" :style="{ padding: '4px' }">
              <SqlPanel />
            </div>
          </template>
          <template #2>
            <div class="h-full w-full overflow-auto">
              <ResultPanel />
            </div>
          </template>
        </NSplit>
      </template>
    </NSplit>
  </div>
</template>

<style scoped></style>

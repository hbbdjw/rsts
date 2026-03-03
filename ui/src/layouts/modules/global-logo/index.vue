<script setup lang="ts">
import { $t } from '@/locales';

defineOptions({
  name: 'GlobalLogo'
});

interface Props {
  /** Whether to show the title */
  showTitle?: boolean;
}

withDefaults(defineProps<Props>(), {
  showTitle: true
});
</script>

<template>
  <RouterLink to="/" class="global-logo-link w-full nowrap-hidden">
    <!-- <SystemLogo class="size-32px" /> -->
    <h2 v-show="showTitle" class="text-16px text-primary font-bold transition duration-300 ease-in-out">
      {{ $t('system.title') }}
    </h2>
  </RouterLink>
</template>

<style scoped>
.global-logo-link {
  display: flex;
  align-items: center;
  justify-content: left;
  margin-left: 12px;
}
</style>

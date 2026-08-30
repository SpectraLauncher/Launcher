<script setup lang="ts">
const updater = useAutoUpdate()
const { t } = useI18n()

// The window controls sit at z-100; this deliberately stays under them so the
// launcher can still be closed while it updates.
const message = computed(() => {
  if (updater.status.value === 'downloading') {
    return t('update.startupDownloading', { version: updater.newVersion.value })
  }
  if (updater.status.value === 'ready') return t('update.startupInstalling')
  return t('update.startupChecking')
})

// Nothing is known about the size until the first chunk arrives, so the bar
// runs indeterminate rather than sitting at a lying 0%.
const known = computed(() => updater.total.value > 0)
</script>

<template>
  <Transition
    enter-active-class="transition duration-200"
    enter-from-class="opacity-0"
    leave-active-class="transition duration-300"
    leave-to-class="opacity-0"
  >
    <div
      v-if="updater.gate.value !== 'done'"
      data-tauri-drag-region
      class="fixed inset-0 z-[90] flex flex-col items-center justify-center gap-6 bg-[#0b0e14]"
    >
      <div
        class="pointer-events-none absolute inset-0"
        style="background-image:radial-gradient(rgba(255,255,255,0.035) 1px,transparent 1px);background-size:26px 26px;"
      />

      <div class="relative flex flex-col items-center gap-5 px-8 text-center">
        <img src="/logo-transparent.png" alt="" class="h-16 w-16 object-contain" >

        <div class="space-y-1.5">
          <p class="text-lg font-semibold text-[#eef1f5]">{{ t('update.startupTitle') }}</p>
          <p class="text-sm text-gray-400">{{ message }}</p>
        </div>

        <div class="h-1 w-64 overflow-hidden rounded-full bg-white/10">
          <div
            v-if="known"
            class="h-full rounded-full bg-primary-500 transition-[width] duration-200"
            :style="{ width: `${updater.progress.value}%` }"
          />
          <div v-else class="h-full w-1/3 animate-[startup-scan_1.2s_ease-in-out_infinite] rounded-full bg-primary-500" />
        </div>

        <p v-if="known" class="text-xs tabular-nums text-gray-500">{{ updater.progress.value }}%</p>
      </div>
    </div>
  </Transition>
</template>

<style>
@keyframes startup-scan {
  0%   { transform: translateX(-100%); }
  100% { transform: translateX(300%); }
}
</style>

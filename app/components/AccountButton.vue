<script setup lang="ts">
const panel = useAccountPanel()
const account = useSpectraAccount()
const notifications = useSpectraNotifications()

const badge = computed(() => notifications.unread.value)
</script>

<template>
  <button
    type="button"
    style="-webkit-app-region: no-drag"
    class="relative flex h-7 items-center gap-1.5 rounded-full border px-2.5 text-xs transition"
    :class="panel.isOpen.value
      ? 'border-primary-500/50 bg-primary-500/15 text-primary-200'
      : 'border-white/8 bg-white/4 text-neutral-300 hover:border-white/20 hover:bg-white/8'"
    :title="$t('spectra.title')"
    @click="panel.toggle()"
  >
    <img
      v-if="account.user.value?.image"
      :src="account.user.value.image"
      alt=""
      class="size-4 shrink-0 rounded-full object-cover"
    >
    <span
      v-else-if="account.isSignedIn.value"
      class="flex size-4 shrink-0 items-center justify-center rounded-full text-[9px] font-bold text-white/90"
      :style="`background:hsl(${spectraInitial(account.displayName.value).hue} 55% 30%)`"
    >{{ spectraInitial(account.displayName.value).letter }}</span>
    <UIcon v-else name="i-lucide-user-round" class="size-3.5 shrink-0" />

    <span v-if="account.isSignedIn.value" class="max-w-24 truncate">{{ account.displayName.value }}</span>
    <span v-else>{{ $t('spectra.signIn') }}</span>

    <span
      v-if="badge"
      class="absolute -top-1 -right-1 flex h-4 min-w-4 items-center justify-center rounded-full bg-primary-500 px-1 text-[10px] font-bold text-[#04121f]"
    >{{ badge > 9 ? '9+' : badge }}</span>
  </button>
</template>

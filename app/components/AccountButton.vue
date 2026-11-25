<script setup lang="ts">
const panel = useAccountPanel()
const account = useSpectraAccount()
const notifications = useSpectraNotifications()

const badge = computed(() => notifications.unread.value)
const { t } = useI18n()
const statusLabel = computed(() => t(`spectra.mode.${account.presence.value}`))
</script>

<template>
  <button
    type="button"
    data-account-toggle
    style="-webkit-app-region: no-drag"
    :aria-label="account.isSignedIn.value ? `${$t('spectra.title')}: ${statusLabel}` : $t('spectra.signIn')"
    class="relative flex h-7 items-center gap-1.5 rounded-full border pl-1 pr-2.5 text-xs transition"
    :class="panel.isOpen.value
 ? 'border-primary/50 bg-primary/15 text-primary-200'
 : 'border-default bg-muted text-neutral-300 hover:border-(--ui-border-accented) hover:bg-elevated'"
    :title="$t('spectra.title')"
    @click="panel.toggle()"
  >
    <span v-if="account.isSignedIn.value" class="relative shrink-0" :title="statusLabel">
      <img
        v-if="account.user.value?.image"
        :src="account.user.value.image"
        alt=""
        class="size-5 rounded-full object-cover"
      >
      <span
        v-else
        class="flex size-5 items-center justify-center rounded-full text-[10px] font-bold text-white/90"
        :style="`background:hsl(${spectraInitial(account.displayName.value).hue} 55% 30%)`"
      >{{ spectraInitial(account.displayName.value).letter }}</span>
      <span
        class="absolute -right-0.5 -bottom-0.5 size-2.5 rounded-full border-2 border-(--mk-chrome)"
        :style="`background:${SPECTRA_STATUS_COLOUR[PRESENCE_STATUS[account.presence.value]]}`"
      />
    </span>
    <UIcon v-else name="i-lucide-user-round" class="size-3.5 shrink-0" />

    <span v-if="account.isSignedIn.value" class="max-w-24 truncate">{{ account.displayName.value }}</span>
    <span v-else>{{ $t('spectra.signIn') }}</span>

    <span
      v-if="badge"
      class="absolute -top-1 -right-1 flex h-4 min-w-4 items-center justify-center rounded-full bg-primary px-1 text-[10px] font-bold text-inverted"
    >{{ badge > 9 ? '9+' : badge }}</span>
  </button>
</template>

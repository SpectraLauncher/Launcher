<template>
  <UModal :open="isOpen" :title="$t('addons.installTitle')" :dismissible="false" :ui="{ content: 'max-w-md' }">
    <template #body>
      <div v-if="loading && !preview" class="flex items-center gap-2 text-sm text-muted">
        <UIcon name="i-lucide-loader-circle" class="size-4 animate-spin" />
        {{ $t('addons.checking') }}
      </div>

      <UAlert v-else-if="error" color="error" variant="subtle" icon="i-lucide-circle-alert" :description="error" />

      <div v-else-if="preview" class="space-y-4">
        <div>
          <div class="flex flex-wrap items-center gap-2">
            <p class="text-lg font-semibold">{{ preview.name }}</p>
            <span class="rounded-md bg-white/7 px-2 py-0.5 font-mono text-xs">{{ preview.version }}</span>
            <UBadge v-if="preview.source !== 'catalog'" color="warning" variant="soft" size="sm" :label="$t('addons.unverified')" />
          </div>
          <p v-if="preview.installed" class="mt-1 text-xs text-muted">{{ $t('addons.updateFrom', { version: preview.installed }) }}</p>
          <p v-if="preview.description" class="mt-2 text-sm text-muted">{{ preview.description }}</p>
        </div>

        <div v-if="preview.themes.length || preview.buttons.length">
          <p class="mb-1.5 text-sm font-medium">{{ $t('addons.adds') }}</p>
          <ul class="space-y-1 text-sm text-muted">
            <li v-for="name in preview.themes" :key="`t-${name}`" class="flex items-center gap-2">
              <UIcon name="i-lucide-palette" class="size-4 shrink-0" />
              {{ $t('addons.addsTheme', { name }) }}
            </li>
            <li v-for="(b, i) in preview.buttons" :key="`b-${i}`" class="flex items-center gap-2">
              <UIcon name="i-lucide-mouse-pointer-click" class="size-4 shrink-0" />
              {{ $t('addons.addsButton', { title: b.title, place: $t(`addons.slots.${slotKey(b.slot)}`) }) }}
            </li>
          </ul>
        </div>

        <div v-if="preview.links.length">
          <p class="mb-1.5 text-sm font-medium">{{ $t('addons.links') }}</p>
          <ul class="space-y-1 font-mono text-xs text-muted">
            <li v-for="host in preview.links" :key="host">{{ host }}</li>
          </ul>
        </div>

        <div>
          <p class="mb-1.5 text-sm font-medium">{{ $t('addons.permissions') }}</p>
          <ul v-if="preview.permissions.length" class="space-y-1 font-mono text-xs text-muted">
            <li v-for="p in preview.permissions" :key="p">{{ p }}</li>
          </ul>
          <p v-else class="text-sm text-muted">{{ $t('addons.noPermissions') }}</p>
        </div>
      </div>
    </template>

    <template #footer>
      <div class="flex w-full justify-end gap-2">
        <UButton color="neutral" variant="ghost" :disabled="loading && !!preview" :label="$t('common.cancel')" @click="cancel" />
        <UButton
          v-if="preview"
          :loading="loading"
          :label="preview.installed ? $t('addons.update') : $t('addons.install')"
          @click="onConfirm"
        />
      </div>
    </template>
  </UModal>
</template>

<script setup lang="ts">
const { isOpen, preview, loading, error, confirm, cancel } = useAddonInstall()
const toast = useToast()
const { t } = useI18n()

const slotKey = (slot: string) => slot.replace(/\.(\w)/g, (_, c: string) => c.toUpperCase())

async function onConfirm() {
  const name = await confirm()
  if (name) toast.add({ title: t('addons.installedToast', { name }), color: 'success' })
}
</script>

<template>
  <UModal v-model:open="isOpen" :title="$t('link.title')" :dismissible="false" :ui="{ content: 'max-w-md' }">
    <template #body>
      <div v-if="req" class="space-y-4">
        <p class="text-sm text-muted">{{ $t('link.desc') }}</p>

        <div>
          <div class="mb-1 flex items-center justify-between text-xs text-muted">
            <span>{{ $t('link.file', { i: index + 1, n: req.files.length }) }}</span>
            <span v-if="processing" class="flex items-center gap-1 text-primary-400">
              <UIcon name="i-lucide-loader-circle" class="size-3.5 animate-spin" /> {{ $t('common.loading') }}
            </span>
          </div>
          <div class="flex items-center gap-2 rounded-lg border border-default bg-white/3 px-3 py-2">
            <UIcon name="i-lucide-file" class="size-4 shrink-0 text-neutral-500" />
            <span class="min-w-0 flex-1 truncate font-mono text-xs">{{ currentFile }}</span>
          </div>
        </div>

        <div class="space-y-2">
          <p class="text-sm font-medium">{{ $t('link.provider') }}</p>
          <div class="flex gap-2">
            <button
              v-for="p in providerOptions"
              :key="p.value"
              type="button"
              class="flex-1 rounded-lg border px-3 py-2 text-sm font-medium transition"
              :class="provider === p.value ? 'border-primary-500 bg-primary-500/10 text-primary-300' : 'border-default text-neutral-300 hover:border-neutral-500'"
              @click="provider = p.value"
            >{{ p.label }}</button>
          </div>
          <UCheckbox v-if="providerOptions.length > 1" v-model="tryOthers" :label="$t('link.tryOthers')" />
        </div>
      </div>
    </template>

    <template #footer>
      <div class="flex w-full flex-wrap items-center justify-between gap-2">
        <div class="flex gap-2">
          <UButton color="neutral" variant="ghost" :disabled="processing" :label="$t('link.skip')" @click="skipOne" />
          <UButton v-if="hasMany" color="neutral" variant="ghost" :disabled="processing" :label="$t('link.skipAll')" @click="finish" />
        </div>
        <div class="flex gap-2">
          <UButton :loading="processing" :label="$t('link.confirm')" @click="confirmOne" />
          <UButton v-if="hasMany" color="neutral" variant="soft" :loading="processing" :label="$t('link.confirmAll')" @click="confirmAll" />
        </div>
      </div>
    </template>
  </UModal>
</template>

<script setup lang="ts">
const { isOpen, req, close } = useLinkModsModal()
const modrinth = useModrinth()
const curseforge = useCurseforge()
const toast = useToast()
const { t } = useI18n()

type Provider = 'modrinth' | 'curseforge'
const index = ref(0)
const provider = ref<Provider>('modrinth')
const tryOthers = ref(true)
const processing = ref(false)
let matched = 0
let skipped = 0

const currentFile = computed(() => req.value?.files[index.value] ?? '')
const hasMany = computed(() => (req.value?.files.length ?? 0) > 1)
const providerOptions = computed(() => {
  const opts: { label: string, value: Provider }[] = [{ label: 'Modrinth', value: 'modrinth' }]
  if (req.value?.cfEnabled) opts.push({ label: 'CurseForge', value: 'curseforge' })
  return opts
})

watch(isOpen, (open) => {
  if (!open) return
  index.value = 0
  provider.value = 'modrinth'
  tryOthers.value = true
  processing.value = false
  matched = 0
  skipped = 0
})

async function matchOne(file: string, prov: Provider, others: boolean): Promise<boolean> {
  const order: Provider[] = prov === 'curseforge' ? ['curseforge', 'modrinth'] : ['modrinth', 'curseforge']
  const chain = others ? order : [prov]
  const id = req.value!.instanceId
  for (const p of chain) {
    if (p === 'curseforge' && !req.value!.cfEnabled) continue
    try {
      const ok = p === 'curseforge'
        ? await curseforge.matchFile(id, file)
        : await modrinth.matchFile(id, file)
      if (ok) return true
    } catch {  }
  }
  return false
}

function advance() {
  if (index.value + 1 >= (req.value?.files.length ?? 0)) {
    finish()
  } else {
    index.value++
  }
}

async function confirmOne() {
  if (!req.value) return
  processing.value = true
  try {
    const ok = await matchOne(currentFile.value, provider.value, tryOthers.value)
    ok ? matched++ : skipped++
  } finally {
    processing.value = false
  }
  advance()
}

async function confirmAll() {
  if (!req.value) return
  processing.value = true
  try {
    const prov = provider.value
    const others = tryOthers.value
    for (let i = index.value; i < req.value.files.length; i++) {
      index.value = i
      const ok = await matchOne(req.value.files[i]!, prov, others)
      ok ? matched++ : skipped++
    }
  } finally {
    processing.value = false
  }
  finish()
}

function skipOne() {
  skipped++
  advance()
}

function finish() {
  const done = req.value?.onDone
  close()
  toast.add({
    title: matched > 0 ? t('link.done', { n: matched }) : t('mods.noMatch'),
    color: matched > 0 ? 'success' : 'neutral',
  })
  done?.()
}
</script>

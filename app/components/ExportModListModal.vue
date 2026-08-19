<template>
  <UModal v-model:open="isOpen" :title="$t('modlist.title')" :ui="{ content: 'max-w-2xl' }">
    <template #body>
      <div class="space-y-4">
        <div class="flex flex-wrap items-center gap-3">
          <USelect v-model="formatKey" :items="formatItems" value-key="value" class="w-40" />
          <div class="flex items-center gap-3 text-sm">
            <UCheckbox v-model="opts.version" :label="$t('modlist.version')" />
            <UCheckbox v-model="opts.url" :label="$t('modlist.url')" />
            <UCheckbox v-model="opts.filename" :label="$t('modlist.filename')" />
          </div>
        </div>

        <UInput v-if="formatKey === 'custom'" v-model="template" class="w-full font-mono text-xs" :placeholder="'- {name} [{version}] ({url})'" />

        <UTextarea :model-value="output" :rows="12" readonly class="w-full font-mono text-xs" />
        <p class="text-xs text-muted">{{ $t('modlist.count', { n: mods.length }) }}</p>
      </div>
    </template>

    <template #footer>
      <div class="flex w-full items-center gap-3">
        <ModalHint v-if="formatKey === 'custom'">
          {{ $t('modlist.customHint') }} <code class="font-mono">{name}</code>, <code class="font-mono">{version}</code>, <code class="font-mono">{url}</code>
        </ModalHint>
        <div class="ml-auto flex shrink-0 gap-2">
          <UButton variant="ghost" color="neutral" :label="$t('common.close')" @click="close" />
          <UButton icon="i-lucide-copy" color="neutral" variant="soft" :label="$t('common.copy')" @click="copy" />
          <UButton icon="i-lucide-download" :label="$t('modlist.save')" @click="saveFile" />
        </div>
      </div>
    </template>
  </UModal>
</template>

<script setup lang="ts">
import { invoke } from '@tauri-apps/api/core'
import { save } from '@tauri-apps/plugin-dialog'
import type { ModEntry } from '~/types/launcher'

const { isOpen, instanceId, close } = useModListModal()
const toast = useToast()
const { t } = useI18n()

type Fmt = 'markdown' | 'html' | 'json' | 'csv' | 'plain' | 'custom'
const formatKey = ref<Fmt>('markdown')
const opts = reactive({ version: true, url: true, filename: false })
const template = ref('- {name} [{version}] ({url})')
const mods = ref<ModEntry[]>([])

const formatItems = computed(() => [
  { label: 'Markdown', value: 'markdown' },
  { label: 'HTML', value: 'html' },
  { label: 'JSON', value: 'json' },
  { label: 'CSV', value: 'csv' },
  { label: t('modlist.plain'), value: 'plain' },
  { label: t('modlist.custom'), value: 'custom' },
])

watch(isOpen, async (open) => {
  if (!open || !instanceId.value) return
  try {
    mods.value = await invoke<ModEntry[]>('list_mods', { instanceId: instanceId.value })
    mods.value.sort((a, b) => (a.name ?? a.filename).toLowerCase().localeCompare((b.name ?? b.filename).toLowerCase()))
  } catch (e) {
    toast.add({ title: String(e), color: 'error' })
  }
})

// Per-mod fields.
const nameOf = (m: ModEntry) => m.name ?? m.filename
const urlOf = (m: ModEntry) => (m.project_id ? `https://modrinth.com/mod/${m.project_id}` : '')
const verOf = (m: ModEntry) => m.version ?? ''

function mdEscape(s: string) {
  return s.replace(/[\\`*_{}[\]<>()#+\-.!|]/g, c => `\\${c}`)
}

const output = computed(() => {
  const ms = mods.value
  switch (formatKey.value) {
    case 'markdown':
      return ms.map((m) => {
        let name = mdEscape(nameOf(m))
        if (opts.url && urlOf(m)) name = `[${name}](${urlOf(m)})`
        let line = name
        if (opts.version && verOf(m)) line += ` [${mdEscape(verOf(m))}]`
        if (opts.filename) line += ` (${mdEscape(m.filename)})`
        return `- ${line}`
      }).join('\n')
    case 'html': {
      const items = ms.map((m) => {
        let name = escapeHtml(nameOf(m))
        if (opts.url && urlOf(m)) name = `<a href="${escapeHtml(urlOf(m))}">${name}</a>`
        let line = name
        if (opts.version && verOf(m)) line += ` [${escapeHtml(verOf(m))}]`
        if (opts.filename) line += ` (${escapeHtml(m.filename)})`
        return `\t<li>${line}</li>`
      }).join('\n')
      return `<html><body><ul>\n${items}\n</ul></body></html>`
    }
    case 'json':
      return JSON.stringify(ms.map((m) => {
        const o: Record<string, string> = { name: nameOf(m) }
        if (opts.url && urlOf(m)) o.url = urlOf(m)
        if (opts.version && verOf(m)) o.version = verOf(m)
        if (opts.filename) o.filename = m.filename
        return o
      }), null, 2)
    case 'csv':
      return ms.map((m) => {
        const cols = [csv(nameOf(m))]
        if (opts.url) cols.push(csv(urlOf(m)))
        if (opts.version) cols.push(csv(verOf(m)))
        if (opts.filename) cols.push(csv(m.filename))
        return cols.join(',')
      }).join('\n')
    case 'plain':
      return ms.map((m) => {
        let line = nameOf(m)
        if (opts.url && urlOf(m)) line += ` (${urlOf(m)})`
        if (opts.version && verOf(m)) line += ` [${verOf(m)}]`
        if (opts.filename) line += ` (${m.filename})`
        return line
      }).join('\n')
    case 'custom':
      return ms.map(m => template.value
        .replaceAll('{name}', nameOf(m))
        .replaceAll('{version}', verOf(m))
        .replaceAll('{url}', urlOf(m))
        .replaceAll('{filename}', m.filename)
        .replaceAll('{mod_id}', m.project_id ?? ''),
      ).join('\n')
  }
  return ''
})

function escapeHtml(s: string) {
  return s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;')
}
function csv(s: string) {
  return /[",\n]/.test(s) ? `"${s.replace(/"/g, '""')}"` : s
}

const extFor: Record<Fmt, string> = { markdown: 'md', html: 'html', json: 'json', csv: 'csv', plain: 'txt', custom: 'txt' }

async function copy() {
  try {
    await navigator.clipboard.writeText(output.value)
    toast.add({ title: t('common.copied'), color: 'success' })
  } catch (e) {
    toast.add({ title: String(e), color: 'error' })
  }
}

async function saveFile() {
  try {
    const dest = await save({ defaultPath: `mods.${extFor[formatKey.value]}` })
    if (typeof dest !== 'string') return
    await invoke('write_text_file', { path: dest, content: output.value })
    toast.add({ title: t('modlist.saved'), color: 'success' })
  } catch (e) {
    toast.add({ title: String(e), color: 'error' })
  }
}
</script>

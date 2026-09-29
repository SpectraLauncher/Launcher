import { invoke } from '@tauri-apps/api/core'
import type { AddonPreview } from '~/types/launcher'

export const useAddonInstall = () => {
  const isOpen = useState('addon-install-open', () => false)
  const preview = useState<AddonPreview | null>('addon-install-preview', () => null)
  const loading = useState('addon-install-loading', () => false)
  const error = useState<string | null>('addon-install-error', () => null)

  async function stage(command: string, args: Record<string, unknown>) {
    preview.value = null
    error.value = null
    loading.value = true
    isOpen.value = true
    try {
      preview.value = await invoke<AddonPreview>(command, args)
    } catch (e) {
      error.value = errorText(e)
    } finally {
      loading.value = false
    }
  }

  const fromCatalog = (slug: string) => stage('addons_stage_catalog', { slug })
  const fromFile = (path: string) => stage('addons_stage_file', { path })
  const fromFolder = (path: string) => stage('addons_stage_folder', { path })

  async function confirm() {
    if (!preview.value) return
    loading.value = true
    try {
      await invoke('addons_commit', { token: preview.value.token })
      await useAddonsStore().load()
      const name = preview.value.name
      preview.value = null
      isOpen.value = false
      return name
    } catch (e) {
      error.value = errorText(e)
      preview.value = null
    } finally {
      loading.value = false
    }
  }

  function cancel() {
    if (preview.value) invoke('addons_discard', { token: preview.value.token }).catch(() => {})
    preview.value = null
    isOpen.value = false
  }

  return { isOpen, preview, loading, error, fromCatalog, fromFile, fromFolder, confirm, cancel }
}

import { ref } from 'vue'
import { platform } from '@tauri-apps/plugin-os'

type OsPlatform = 'windows' | 'macos' | 'linux' | 'unknown'

const current = ref<OsPlatform>('unknown')
let detecting: Promise<void> | null = null

export function usePlatform() {
  if (!detecting) {
    detecting = (async () => {
      try {
        const os = await platform()
        current.value = os === 'windows' ? 'windows' : os === 'macos' ? 'macos' : 'linux'
      } catch {
        current.value = 'unknown'
      }
    })()
  }

  return {
    platform: current,
    ready: detecting,
  }
}

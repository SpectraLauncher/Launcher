import { invoke } from '@tauri-apps/api/core'

export interface JavaInstallation {
  path: string
  version?: string
  major?: number
  vendor?: string
  arch?: string
  is_valid: boolean
}

export interface JavaValidation {
  is_valid: boolean
  version?: string
  major?: number
  vendor?: string
  arch?: string
  error?: string
}

export const useJava = () => {
  const installations = useState<JavaInstallation[]>('java-installations', () => [])
  const scanning = useState('java-scanning', () => false)

  const scan = async () => {
    scanning.value = true
    try {
      installations.value = await invoke<JavaInstallation[]>('detect_java_installations')
    } catch {
      installations.value = []
    } finally {
      scanning.value = false
    }
  }

  const validate = (path: string) => invoke<JavaValidation>('validate_java_path', { path })

  return { installations, scanning, scan, validate }
}

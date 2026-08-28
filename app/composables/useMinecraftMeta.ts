import { invoke } from '@tauri-apps/api/core'
import type { LoaderType } from '~/types/launcher'

export interface MinecraftVersion {
  id: string
  kind: string
  release_time: string
}

export interface LoaderVersion {
  version: string
  stable: boolean
}

export type LoaderVersionMode = 'stable' | 'latest' | 'other'

export const useMinecraftMeta = () => {
  const getMinecraftVersions = (includeSnapshots = false) =>
    invoke<MinecraftVersion[]>('get_minecraft_versions', { includeSnapshots })

  const getLoaderVersions = (loader: LoaderType, mcVersion: string) =>
    invoke<LoaderVersion[]>('get_loader_versions', { loader, mcVersion })

  const resolveLoaderVersion = async (
    loader: LoaderType,
    mcVersion: string,
    mode: LoaderVersionMode,
    explicit?: string,
  ): Promise<string> => {
    if (loader === 'vanilla') return ''
    if (mode === 'other') {
      if (!explicit) throw new Error('no loader version selected')
      return explicit
    }
    const versions = await getLoaderVersions(loader, mcVersion)
    if (!versions.length) throw new Error(`no ${loader} versions for ${mcVersion}`)
    if (mode === 'stable') {
      return (versions.find(v => v.stable) ?? versions[0]).version
    }
    return versions[0].version
  }

  return { getMinecraftVersions, getLoaderVersions, resolveLoaderVersion }
}

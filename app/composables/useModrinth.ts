import { invoke } from '@tauri-apps/api/core'
import type { Instance } from '~/types/launcher'
import type {
  ModrinthSearchParams,
  ModrinthSearchResponse,
  ModrinthVersion,
  ModrinthCategory,
  ModrinthProjectType,
  ModrinthProjectFull,
  InstalledItem,
  ModUpdate,
  ModpackUpdate,
} from '~/types/modrinth'

export const useModrinth = () => {
  const search = (params: ModrinthSearchParams) =>
    invoke<ModrinthSearchResponse>('modrinth_search', { params })

  const versions = (projectId: string, loaders?: string[], gameVersions?: string[]) =>
    invoke<ModrinthVersion[]>('modrinth_versions', {
      projectId,
      loaders: loaders ?? null,
      gameVersions: gameVersions ?? null,
    })

  const project = (id: string) => invoke<ModrinthProjectFull>('modrinth_project', { id })

  const categories = (projectType: ModrinthProjectType) =>
    invoke<ModrinthCategory[]>('modrinth_categories', { projectType })

  const installWithDeps = (
    instanceId: string,
    versionId: string,
    gameVersion?: string,
    loader?: string,
  ) =>
    invoke<InstalledItem[]>('modrinth_install_with_deps', {
      instanceId,
      versionId,
      gameVersion: gameVersion ?? null,
      loader: loader ?? null,
    })

  const getInstalled = (instanceId: string) =>
    invoke<InstalledItem[]>('get_installed_content', { instanceId })

  const matchLocal = (instanceId: string) =>
    invoke<number>('match_local_mods', { instanceId })

  const matchFile = (instanceId: string, filename: string) =>
    invoke<boolean>('modrinth_match_file', { instanceId, filename })

  const updateAll = (instanceId: string, loaders?: string[], gameVersions?: string[]) =>
    invoke<number>('update_all_mods', {
      instanceId,
      loaders: loaders ?? null,
      gameVersions: gameVersions ?? null,
    })

  const checkUpdates = (instanceId: string, loaders?: string[], gameVersions?: string[]) =>
    invoke<ModUpdate[]>('check_mod_updates', {
      instanceId,
      loaders: loaders ?? null,
      gameVersions: gameVersions ?? null,
    })

  const installModpack = (
    url: string,
    nameOverride?: string | null,
    iconUrl?: string | null,
    projectId?: string | null,
    versionId?: string | null,
  ) =>
    invoke<Instance>('modrinth_install_modpack', {
      url,
      nameOverride: nameOverride ?? null,
      iconUrl: iconUrl ?? null,
      projectId: projectId ?? null,
      versionId: versionId ?? null,
    })

  const checkModpackUpdate = (instanceId: string) =>
    invoke<ModpackUpdate | null>('check_modpack_update', { instanceId })

  const updateModpack = (instanceId: string) =>
    invoke<void>('update_modpack', { instanceId })

  return { search, versions, project, categories, installWithDeps, getInstalled, matchLocal, matchFile, checkUpdates, updateAll, installModpack, checkModpackUpdate, updateModpack }
}

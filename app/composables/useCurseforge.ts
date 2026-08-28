import { invoke } from '@tauri-apps/api/core'
import type { Instance, BlockedMod } from '~/types/launcher'
import type {
  ModrinthSearchParams,
  ModrinthSearchResponse,
  ModrinthVersion,
  ModrinthProjectFull,
  ModrinthCategory,
  InstalledItem,
} from '~/types/modrinth'

export interface CfInstallResult {
  added: InstalledItem[]
  blocked: BlockedMod[]
}

export const useCurseforge = () => {
  const enabled = () => invoke<boolean>('cf_enabled')

  const search = (params: ModrinthSearchParams) =>
    invoke<ModrinthSearchResponse>('curseforge_search', { params })

  const versions = (projectId: string, loaders?: string[], gameVersions?: string[]) =>
    invoke<ModrinthVersion[]>('curseforge_versions', {
      projectId,
      loaders: loaders ?? null,
      gameVersions: gameVersions ?? null,
    })

  const project = (id: string) => invoke<ModrinthProjectFull>('curseforge_project', { id })

  const categories = (projectType: string) =>
    invoke<ModrinthCategory[]>('curseforge_categories', { projectType })

  const installWithDeps = (
    instanceId: string,
    projectId: string,
    fileId: string,
    gameVersion?: string,
    loader?: string,
  ) =>
    invoke<CfInstallResult>('curseforge_install_with_deps', {
      instanceId,
      projectId,
      fileId,
      gameVersion: gameVersion ?? null,
      loader: loader ?? null,
    })

  const matchLocal = (instanceId: string) =>
    invoke<number>('curseforge_match_local', { instanceId })

  const matchFile = (instanceId: string, filename: string) =>
    invoke<boolean>('curseforge_match_file', { instanceId, filename })

  const getBlocked = (instanceId: string) =>
    invoke<BlockedMod[]>('get_blocked_mods', { instanceId })

  const updateAll = (instanceId: string, loaders?: string[], gameVersions?: string[]) =>
    invoke<number>('curseforge_update_all', {
      instanceId,
      loaders: loaders ?? null,
      gameVersions: gameVersions ?? null,
    })

  const installModpack = (projectId: string, fileId: string, nameOverride?: string | null) =>
    invoke<Instance>('curseforge_install_modpack', {
      projectId,
      fileId,
      nameOverride: nameOverride ?? null,
    })

  return { enabled, search, versions, project, categories, installWithDeps, matchLocal, matchFile, getBlocked, updateAll, installModpack }
}

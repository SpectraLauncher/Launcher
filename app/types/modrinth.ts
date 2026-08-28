export type ModrinthProjectType = 'mod' | 'modpack' | 'resourcepack' | 'shader'

export type ContentKind = 'mod' | 'modpack' | 'resourcepack' | 'shader' | 'datapack'

export type ModrinthSortIndex = 'relevance' | 'downloads' | 'follows' | 'newest' | 'updated'

export interface ModrinthSearchParams {
  query: string
  project_type: ModrinthProjectType
  loaders?: string[]
  game_versions?: string[]
  categories?: string[]
  index?: ModrinthSortIndex
  offset?: number
  limit?: number
}

export interface ModrinthHit {
  project_id: string
  slug: string
  title: string
  description: string
  author: string
  project_type: string
  downloads: number
  follows: number
  icon_url: string | null
  categories: string[]
  versions: string[]
  client_side: string | null
  server_side: string | null
}

export interface ModrinthSearchResponse {
  hits: ModrinthHit[]
  total_hits: number
  offset: number
  limit: number
}

export interface ModrinthVersionFile {
  url: string
  filename: string
  primary: boolean
  size: number
  hashes: { sha1: string | null; sha512: string | null }
}

export interface ModrinthDependency {
  project_id: string | null
  version_id: string | null
  dependency_type: string
}

export interface ModrinthVersion {
  id: string
  project_id: string
  name: string
  version_number: string
  version_type: 'release' | 'beta' | 'alpha'
  loaders: string[]
  game_versions: string[]
  downloads: number
  date_published: string
  files: ModrinthVersionFile[]
  dependencies: ModrinthDependency[]
}

export interface ModrinthCategory {
  name: string
  header: string
}

export interface ModrinthGalleryItem {
  url: string
  raw_url: string | null
  title: string | null
  description: string | null
  featured: boolean
}

export interface ModrinthProjectFull {
  id: string
  title: string
  description: string
  body: string
  icon_url: string | null
  gallery: ModrinthGalleryItem[]
}

export interface ModUpdate {
  project_id: string
  version_id: string
  version_number: string
}

export interface ModpackUpdate {
  version_id: string
  version_number: string
  version_type: 'release' | 'beta'
  changelog: string
  date_published: string
}

export interface InstalledItem {
  project_id: string
  version_id: string
  kind: ContentKind
  name: string
  filename: string
  version_number: string
  icon_url: string | null
  game_versions: string[]
  loaders: string[]
  dependency: boolean
  installed_at: string
}

import type { LastJoined, QuickPlay } from '~/types/launcher'

export function supportsQuickPlay(version: string): boolean {
  const snapshot = /^(\d{2})w(\d{2})[a-z]$/.exec(version)
  if (snapshot) return Number(snapshot[1]) * 100 + Number(snapshot[2]) >= 2314
  const [major = 0, minor = 0] = version.split(/[.-]/).map(part => Number.parseInt(part, 10) || 0)
  return major > 1 || (major === 1 && minor >= 20)
}

export function quickPlayFor(joined: LastJoined): QuickPlay {
  return joined.kind === 'Singleplayer'
    ? { kind: 'Singleplayer', world: joined.world }
    : { kind: 'Multiplayer', host: joined.host, port: joined.port ?? undefined }
}

export function lastJoinedLabel(joined: LastJoined): string {
  if (joined.kind === 'Singleplayer') return joined.name || joined.world
  return joined.port && joined.port !== 25565 ? `${joined.host}:${joined.port}` : joined.host
}

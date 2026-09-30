type Translate = (key: string, params?: Record<string, unknown>) => string

export function addonPermissionLabel(t: Translate, permission: string): string {
  if (permission.startsWith('network:')) return t('addons.permissionNetwork', { host: permission.slice(8) })
  const files = permission.match(/^files:(read|write):(.+)$/)
  if (files) return t(files[1] === 'read' ? 'addons.permissionFilesRead' : 'addons.permissionFilesWrite', { folder: files[2] })
  return t(`addons.permissionNames.${permission.replace(/:(\w)/, (_, c: string) => c.toUpperCase())}`)
}

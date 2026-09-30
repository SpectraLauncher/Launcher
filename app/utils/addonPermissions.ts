type Translate = (key: string, params?: Record<string, unknown>) => string

export function addonPermissionLabel(t: Translate, permission: string): string {
  if (permission.startsWith('network:')) return t('addons.permissionNetwork', { host: permission.slice(8) })
  return t(`addons.permissionNames.${permission.replace(/:(\w)/, (_, c: string) => c.toUpperCase())}`)
}

let resolvedCb: (() => void) | null = null

export const useBlockedModsModal = () => {
  const isOpen = useState('blocked-open', () => false)
  const instanceId = useState<string | null>('blocked-instance', () => null)

  const open = (id: string, onResolved?: () => void) => {
    instanceId.value = id
    resolvedCb = onResolved ?? null
    isOpen.value = true
  }
  const close = () => {
    isOpen.value = false
  }
  const notifyResolved = () => resolvedCb?.()

  return { isOpen, instanceId, open, close, notifyResolved }
}

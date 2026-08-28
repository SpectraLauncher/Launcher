export const useModListModal = () => {
  const isOpen = useState('modlist-modal-open', () => false)
  const instanceId = useState<string | null>('modlist-modal-instance', () => null)

  const open = (id: string) => {
    instanceId.value = id
    isOpen.value = true
  }
  const close = () => {
    isOpen.value = false
  }

  return { isOpen, instanceId, open, close }
}

/**
 * Global open/close state for the shared <ShareInstanceModal>, mirroring
 * useExportModal. Open it with the instance to share.
 */
export const useShareModal = () => {
  const isOpen = useState('share-modal-open', () => false)
  const target = useState<{ id: string, name: string } | null>('share-modal-target', () => null)

  const open = (id: string, name: string) => {
    target.value = { id, name }
    isOpen.value = true
  }
  const close = () => {
    isOpen.value = false
  }

  return { isOpen, target, open, close }
}

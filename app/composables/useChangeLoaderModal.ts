export const useChangeLoaderModal = () => {
  const isOpen = useState('change-loader-open', () => false)
  const instanceId = useState<string | null>('change-loader-instance', () => null)

  const open = (id: string) => {
    instanceId.value = id
    isOpen.value = true
  }
  const close = () => {
    isOpen.value = false
  }

  return { isOpen, instanceId, open, close }
}

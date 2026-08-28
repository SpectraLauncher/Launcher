export const useCreateInstanceModal = () => {
  const isOpen = useState('create-instance-open', () => false)
  const pendingCode = useState<string | null>('create-instance-code', () => null)

  const open = () => {
    isOpen.value = true
  }
  const openWithCode = (code: string) => {
    pendingCode.value = code
    isOpen.value = true
  }
  const close = () => {
    isOpen.value = false
  }

  return { isOpen, pendingCode, open, openWithCode, close }
}

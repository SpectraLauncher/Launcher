/**
 * Shared open/close state for the create-instance modal, so the sidebar button
 * and the globally-mounted <CreateInstanceModal /> can talk to each other.
 */
export const useCreateInstanceModal = () => {
  const isOpen = useState('create-instance-open', () => false)
  // A `spectra://share/<code>` link opens the modal straight on the import step
  // with the code filled in; the modal clears this once it has picked it up.
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

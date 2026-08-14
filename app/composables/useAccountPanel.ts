/**
 * Open/close state for the right-hand account panel, mirroring the other
 * modal composables. Collapsed it renders nothing at all — no rail, no strip.
 */
export const useAccountPanel = () => {
  const isOpen = useState('account-panel-open', () => false)

  return {
    isOpen,
    open: () => { isOpen.value = true },
    close: () => { isOpen.value = false },
    toggle: () => { isOpen.value = !isOpen.value },
  }
}

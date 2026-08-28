export interface LinkModsRequest {
  instanceId: string
  files: string[]
  cfEnabled: boolean
  onDone?: () => void
}

export const useLinkModsModal = () => {
  const isOpen = useState('linkmods-open', () => false)
  const req = useState<LinkModsRequest | null>('linkmods-req', () => null)

  const open = (r: LinkModsRequest) => {
    req.value = r
    isOpen.value = true
  }
  const close = () => {
    isOpen.value = false
  }

  return { isOpen, req, open, close }
}

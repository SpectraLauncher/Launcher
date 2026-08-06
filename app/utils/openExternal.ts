import { invoke } from '@tauri-apps/api/core'

/**
 * Opens a link in the user's default browser.
 *
 * Deliberately not `@tauri-apps/plugin-shell`: inside an AppImage its helper
 * inherits the image's LD_LIBRARY_PATH/GIO_MODULE_DIR and dies silently, so
 * clicking a link appeared to do nothing on Linux. The Rust side strips those
 * before spawning the opener (see instances.rs `system_command`).
 */
export function openExternal(url: string): Promise<void> {
  return invoke('open_external', { url })
}

/**
 * Where the website lives, for the parts of the launcher that talk to it from
 * the frontend.
 *
 * The Rust side has its own copy in `commands/spectra.rs` — the two cannot share
 * a constant across the FFI boundary, and two is still better than the five
 * separate literals this replaces. Anything that builds a site address goes
 * through here so a domain change is one edit per side, not a hunt.
 *
 * Prefer an address the server sent over one built here: the site owns the shape
 * of its own URLs, and /api/shares returns a ready `url` for exactly that reason.
 */
export const SPECTRA_SITE = 'https://usespectra.app'

export const spectraApi = (path: string) => `${SPECTRA_SITE}${path}`

/** The page that opens an instance share. Only a fallback — see the note above. */
export const spectraShareUrl = (code: string) =>
  `${SPECTRA_SITE}/s/${encodeURIComponent(code)}`

# Contributing to Spectra Launcher

Thanks for your interest in improving Spectra Launcher. Bug reports, ideas, translations, testing and code are all
welcome.

## Ways to help

- **Report bugs** with the [bug report form](https://github.com/SpectraLauncher/Launcher/issues/new?template=bug_report.yml).
  Include launcher logs and the crash report when the game crashes.
- **Suggest features** with the [feature request form](https://github.com/SpectraLauncher/Launcher/issues/new?template=feature_request.yml).
- **Translate** the launcher, see [Translations](#translations).
- **Test** new releases with your favorite modpacks and mod loaders.

## Development setup

You need:

- [Rust](https://rustup.rs) stable,
- [Node.js](https://nodejs.org) 24 and [pnpm](https://pnpm.io),
- the [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/) for your operating system.

```bash
pnpm install
pnpm tauri dev
```

The optional `DISCORD_CLIENT_ID` can be placed in `src-tauri/.env`. Without it the launcher builds, but Discord status
is unavailable.

## Project layout

- `src-tauri/src/commands/` contains one module per domain. Every `#[tauri::command]` is also registered in
  `src-tauri/src/lib.rs`.
- `app/` is the Nuxt frontend: `components/`, `composables/`, `pages/`, `stores/` (Pinia) and `utils/`.
- `i18n/locales/` holds the translations.

## Checks

Run these before opening a pull request:

```bash
pnpm generate
cd src-tauri
cargo check
cargo test --lib
```

## Translations

Texts live in `i18n/locales/<language>.json`. There is no fallback locale, so a new key must be added to **every**
locale file. When adding a language, register it in the i18n configuration in `nuxt.config.ts` as well.

## Pull requests

1. Open an issue first for larger changes so the approach can be agreed on.
2. Create a branch from `main` and keep each pull request focused on one change.
3. Write commit messages that say what the change does and why.
4. Fill in the pull request template, including how you tested the change.
5. Changes players will notice get a short entry in `release.txt`, written for players rather than developers.

By contributing you agree that your contributions are licensed under the [GNU General Public License v3.0](../LICENCE).

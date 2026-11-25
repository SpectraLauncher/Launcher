# Product

<!-- impeccable:product-schema 1 -->

## Platform

web

A Tauri 2 desktop app (Windows, macOS, Linux) whose UI is a Nuxt 4 SPA in the system webview, with its own window controls and titlebar. The design language is the launcher's own, not native per OS.

## Users

Two audiences with equal weight:

- **Everyday players** who want to play Minecraft with mods, shaders and modpacks without touching files, folders or Java.
- **Power users**: people who juggle many instances, build or tweak modpacks, move between computers, or develop their own mods and test them on another machine.

Neither group is secondary. Simple paths must stay simple for the first group; depth (per-instance settings, logs, snapshots, configs, cloud sync, sharing) must stay reachable for the second without being buried.

## Product Purpose

Spectra Launcher starts Minecraft and handles the tedious parts: installing versions and loaders, managing Java, keeping each setup in its own **instance**, and installing content from Modrinth and CurseForge with dependencies. Success means a player goes from download to playing a modded game without reading a guide, and a power user never has to leave the launcher to manage files by hand.

## Positioning

What Spectra claims over Prism, the Modrinth app and CurseForge:

- **Simple and good-looking**: fast, modern, no file handling; themes and a user-chosen accent color.
- **Modrinth and CurseForge in one place**, plus import from other launchers and export as `.mrpack` or `.zip`.
- **Spectra accounts and cloud sync**: instances and their Modrinth/CurseForge mods follow you across computers; share codes let others open a pack from the website.
- **Addons**: third-party addons add buttons, pages and windows inside the launcher.

## Operating Context

- Desktop window, used before and between play sessions; launching a game is the core loop.
- Main areas: instance list and instance page (one Content tab for mods/resourcepacks/shaders/datapacks, game files, logs, configs, snapshots, settings behind a gear), content browser, Worlds, Screenshots, Skins (3D preview via skinview3d), live console, Settings.
- Sign-in: Microsoft/Xbox account for Minecraft, plus offline accounts; separate optional Spectra account (signed in from the website via `spectra://` deep link) for sync and sharing.
- Companion website (usespectra.app, Spectra-Web) handles Spectra accounts, share codes and telemetry ingest.
- Silent auto-updater; releases ship as "update X.Y.Z" with plain-language notes in `release.txt`.

## Capabilities and Constraints

- Loaders: Vanilla, Fabric, Quilt, Forge, NeoForge. Install and launch run through the Rust `lyceris` crate, which also manages Java.
- Extras: Quick Play (1.20+), server list with status and ping, crash report detection and sharing, live logs, Discord rich presence, playtime tracking.
- **Must keep working in all future design work:**
  - **Themes and accent color**: Dark, OLED (pure black) and Squared (no rounded corners; formerly "Zębatkowo"), each with a user-picked accent color. Addon themes can also set the mode, accent, a neutral tint and a background image. Every surface has to read correctly in all of them.
  - **Seven languages**: en, pl, de, es, fr, ru, zh. Layouts must survive long German and Russian strings and CJK text; no copy baked into images.
  - **Addon UI slots**: addon-supplied buttons (sidebar, icon and labelled variants), pages and windows render inside the launcher's own UI and must keep a place to land.
- Instance page layout previously took cues from the Modrinth app; that is a past reference, not a binding constraint.
- Not yet code-signed on Windows or macOS (SmartScreen and Gatekeeper warnings are documented in the README).
- Linux packaging (e.g. Nobara RPM) has open issues around opening folders and the browser.

## Brand Commitments

- Name: **Spectra Launcher** (renamed from Mako in June 2026). The name comes from the spectrum gradient glow in the cube logo.
- Logo assets: `public/logo.png`, `public/logo-transparent.png`; instance icon set in `public/instance-symbols/`.
- Voice: plain, friendly language written from the player's side. Explain what changed for the user and why, not the implementation. Mod-loader and Minecraft terms are used as players say them ("instance", "modpack", "shaders").
- Privacy stance: Microsoft sign-in happens directly with Microsoft; accounts, settings and instances stay on the user's computer unless they turn on sync; optional extras are off by default; telemetry is anonymous and opt-in; crash reports are only sent when the user chooses.
- Licence AGPL-3.0-only, made by MakotoPD.

## Evidence on Hand

- Real screenshots: `https://usespectra.app/screenshots/` (MainMenu, InstanceScreen, mod browser for Modrinth and CurseForge, WorldsScreen, SkinsScreen, Settings, InstanceSettings).
- README feature descriptions and the release history in `release.txt`.
- No testimonials, user counts, download numbers, press or reviews exist. Do not invent any.

## Product Principles

1. **Play first.** The shortest path from opening the launcher to being in the game wins over everything else on screen.
2. **Depth without clutter.** Power features stay one deliberate step away, never in the everyday player's path and never hidden so deep a power user can't find them.
3. **Never make people touch files.** If a task would send someone to a folder, the launcher should do it or explain exactly what's needed when it can't.
4. **Say what happened.** Failures (downloads, sign-in, opening folders) surface a plain reason, never a silent no-op.
5. **The user's setup is theirs.** Themes, accent color, language and addons are first-class; designs adapt to them, not the other way round.

---
name: Spectra Launcher
description: A calm graphite Minecraft launcher where the library and the Play button are the first things you see.
colors:
  accent: "oklch(74.6% 0.16 232.661)"
  stop-red: "oklch(70.4% 0.191 22.216)"
  chrome: "#0f1013"
  ground: "#16181c"
  surface: "#1d2024"
  raised: "#23262b"
  pressed: "#31353c"
  hairline: "#262a2f"
  hairline-muted: "#1f2226"
  hairline-strong: "#31353c"
  text-highlighted: "#ffffff"
  text-default: "#d4d7dc"
  text-toned: "#b3b8bf"
  text-muted: "#8d939c"
  text-dimmed: "#7a808a"
  text-inverted: "#16181c"
  oled-ground: "#000000"
  oled-surface: "#0a0a0b"
  oled-raised: "#121315"
  oled-hairline: "#1c1d20"
typography:
  page-title:
    fontFamily: "Inter, system-ui, sans-serif"
    fontSize: "22px"
    fontWeight: 600
    lineHeight: 1.3
    letterSpacing: "-0.01em"
  section-title:
    fontFamily: "Inter, system-ui, sans-serif"
    fontSize: "15px"
    fontWeight: 600
    lineHeight: 1.4
  body:
    fontFamily: "Inter, system-ui, sans-serif"
    fontSize: "14px"
    fontWeight: 400
    lineHeight: 1.5
    fontFeature: "'cv11', 'ss01'"
  meta:
    fontFamily: "Inter, system-ui, sans-serif"
    fontSize: "13px"
    fontWeight: 400
    lineHeight: 1.45
    fontFeature: "'tnum'"
  label:
    fontFamily: "Inter, system-ui, sans-serif"
    fontSize: "13px"
    fontWeight: 500
    lineHeight: 1.4
  column-head:
    fontFamily: "Inter, system-ui, sans-serif"
    fontSize: "11px"
    fontWeight: 500
    lineHeight: 1.4
    letterSpacing: "0.025em"
  mono:
    fontFamily: "JetBrains Mono, ui-monospace, monospace"
    fontSize: "12px"
    fontWeight: 400
    lineHeight: 1.5
rounded:
  base: "4px"
  md: "6px"
  lg: "8px"
  xl: "12px"
  2xl: "16px"
  full: "9999px"
  squared: "0px"
spacing:
  1: "4px"
  2: "8px"
  3: "12px"
  4: "16px"
  5: "20px"
  6: "24px"
  7: "28px"
  8: "32px"
components:
  button-play:
    backgroundColor: "{colors.accent}"
    textColor: "{colors.text-inverted}"
    typography: "{typography.label}"
    rounded: "{rounded.lg}"
    padding: "10px 28px"
  button-stop:
    backgroundColor: "{colors.stop-red}"
    textColor: "{colors.text-inverted}"
    rounded: "{rounded.lg}"
    padding: "8px 20px"
  button-soft:
    backgroundColor: "{colors.raised}"
    textColor: "{colors.text-default}"
    rounded: "{rounded.lg}"
    padding: "8px 14px"
  play-quiet:
    textColor: "{colors.accent}"
    rounded: "{rounded.lg}"
    size: "36px"
  play-quiet-hover:
    backgroundColor: "{colors.accent}"
    textColor: "{colors.text-inverted}"
  filter-chip:
    backgroundColor: "{colors.raised}"
    textColor: "{colors.text-toned}"
    typography: "{typography.label}"
    rounded: "{rounded.full}"
    height: "32px"
    padding: "0 14px"
  filter-chip-selected:
    backgroundColor: "{colors.accent}"
    textColor: "{colors.text-inverted}"
  instance-card:
    backgroundColor: "{colors.surface}"
    textColor: "{colors.text-highlighted}"
    rounded: "{rounded.xl}"
    padding: "12px"
  instance-card-hover:
    backgroundColor: "{colors.raised}"
  segmented-control:
    backgroundColor: "{colors.surface}"
    textColor: "{colors.text-muted}"
    rounded: "{rounded.lg}"
    padding: "2px"
  segmented-option-selected:
    backgroundColor: "{colors.pressed}"
    textColor: "{colors.text-highlighted}"
    rounded: "{rounded.md}"
    padding: "4px 10px"
  rail-item:
    backgroundColor: "{colors.chrome}"
    textColor: "{colors.text-muted}"
    rounded: "{rounded.xl}"
    size: "40px"
  rail-item-hover:
    backgroundColor: "{colors.raised}"
    textColor: "{colors.text-highlighted}"
  search-input:
    backgroundColor: "{colors.raised}"
    textColor: "{colors.text-default}"
    rounded: "{rounded.md}"
    height: "28px"
    width: "288px"
  tab:
    textColor: "{colors.text-muted}"
    typography: "{typography.body}"
    padding: "10px 12px"
  tab-active:
    textColor: "{colors.text-highlighted}"
---

# Design System: Spectra Launcher

## Overview

**Creative North Star: "The Quiet Workbench"**

Spectra is the category standard done carefully, held to the craft of the Modrinth App and Linear. Everything is graphite: a near-black chrome frame, a ground one step lighter, cards half a step above that, and anything raised a full step above. Hairline borders do the separating; shadows are reserved for things that float over the page. Against that calm, one user-chosen accent marks what matters: what is selected, what has focus, and Play.

The library comes first and Play is never more than one click away. Power-user surfaces (content tables, logs, configs, settings) are dense but calm: neutral badges, quiet row actions, colour only where a state changes. The system belongs to the user as much as to Spectra. It ships in three modes (Dark, OLED, Squared), with any of twelve accents and optional addon tints and background images, so every rule below has to hold in all of them.

This replaces an earlier look built on glass cards, a dot-grid background, Space Grotesk and a fixed green Play button. None of that carries over.

**Key Characteristics:**
- Four graphite steps (chrome, ground, surface, raised) separated by hairlines, not shadows.
- One accent, picked by the user, for selection, focus and Play.
- Red appears only for Stop and destructive actions.
- Inter everywhere with tabular figures for data; mono only where the text is machine text.
- Rounded by default (8px controls, 12px cards); Squared mode takes every corner to zero and nothing breaks.

## Colors

A graphite neutral ramp with one swappable accent and one reserved alarm colour.

### Primary
- **Player Accent** (sky by default): the user's chosen accent. It is the fill of the solid Play button and the selected filter chip, the underline on the active tab, the tint on active rail items (15% fill plus accent icon), the ring on a selected theme or instance, the text caret, text selection (35% mix) and focus rings. The accent is picked from twelve Tailwind hues (sky, blue, indigo, violet, purple, pink, orange, amber, green, emerald, teal, cyan) and fed into Nuxt UI's `primary`, so components should reference the primary role and never a specific hue.

### Secondary
- **Stop Red** (Nuxt UI `error`, red-400 in dark): Stop on a running instance, destructive confirms, and a delete icon that turns red on hover. Error text uses it too.

### Neutral
- **Chrome Black** (`--mk-chrome`, neutral-950): the titlebar and icon rail, the frame around the content area.
- **Graphite Ground** (`--ui-bg`, neutral-900): the content area behind every page.
- **Card Surface** (`--ui-bg-muted`, a 55% mix of 900 into 800): cards, table headers, the segmented control track. Half a step above ground.
- **Raised Graphite** (`--ui-bg-elevated`, neutral-800): hover state of cards, chips at rest, soft buttons, modals, tooltips, popovers.
- **Pressed Graphite** (`--ui-bg-accented`, neutral-700): the selected option in a segmented control, scrollbar thumbs.
- **Hairline** (`--ui-border`): the default 1px border on cards, tables and the content frame. **Strong Hairline** (`--ui-border-accented`, neutral-700) is the hover border and the dashed border on empty and add-new states.
- **Text ladder**: highlighted (white) for names and titles, default for body, toned for chips and secondary labels, muted for meta lines, dimmed for counts, icons and placeholders.

OLED mode replaces the ground with true black (#000000) and lifts surface, raised and hairlines just enough to stay separate. An addon tint can replace the whole neutral ramp through `color-mix` in oklab, so neutrals must always come from the ramp and never from fixed hex values.

### Named Rules

**The One Accent Rule.** The accent only marks selection, focus and Play. It is never used for decoration, section headers or illustration. A screen has at most one solid accent Play button. Every other Play is quiet: an accent icon on a 15% accent tint that fills solid on hover or focus.

**The Red Means Stop Rule.** Red is only for stopping a running game and for destructive actions. That is why red and rose are left out of the accent picker. Destructive controls stay neutral at rest and turn red on hover or in the confirm step.

**The Ramp-Only Neutrals Rule.** Grey always comes from the neutral ramp or the `--ui-*` tokens, so OLED and addon tints recolour it correctly.

## Typography

**Body Font:** Inter (with system-ui, sans-serif), with `cv11` and `ss01` turned on
**Mono Font:** JetBrains Mono (with ui-monospace)

**Character:** One neutral sans used across the whole UI, set tight and medium-weight for UI labels. Hierarchy comes from size and weight steps, not from a second display face.

### Hierarchy
- **Page title** (600, 22px, -0.01em): one per page (Library, Settings, the instance name). It sits next to a muted tabular count where there is one.
- **Section title** (600, 15px): section headings within a page, such as Continue playing and library groups.
- **Body** (400, 14px, 1.5): the base size for rows, tabs and settings labels.
- **Meta** (400, 13px, tabular figures): subtitles under names (version, loader, playtime, dates) and counts.
- **Label** (500, 13px): chips, segmented options and button labels.
- **Column head** (500, 11px, uppercase, +0.025em): content table column headers only.
- **Mono** (JetBrains Mono, about 12px): logs, file paths, server addresses, config files and share codes.

### Named Rules

**The Machine Text Rule.** Mono is only for text a machine wrote or reads: logs, paths, addresses, code, config. UI copy never uses it.

**The Tabular Data Rule.** Counts, versions, playtime, dates and progress always use tabular figures so columns and counters don't jitter.

## Layout

The app is a fixed shell. At the top is a 40px titlebar in chrome black that is also the app bar: logo over the rail column, a 288px library search with a `Ctrl K` hint, a drag region, addon titlebar icons, activity, account and window controls. On the left is a 64px icon rail (Home, Worlds, Screenshots, Skins, addon rail buttons, a divider, recent instance icons, a dashed New instance button, addon footer buttons, and Settings pinned at the bottom). The content area sits inside that L-shaped frame as a ground panel with a 12px top-left corner and hairline top and left borders.

Every page shares one gutter: content is centred at a max width of 1400px with 32px side padding and 28px top padding. Sections are separated by 32px. A section title sits 12px above its content. Cards sit in grids with a 12px gap, and page headers wrap their actions with a 16px gap so long German and Russian strings can reflow.

The instance page puts its tab bar in a sticky strip directly under the header. Browse tabs sit on the left and manage tabs on the right. Once the header has scrolled away, a compact identity (24px icon and name) and a small Play slide into the strip. The active tab is kept in the URL, and so is the active Settings section. Settings uses a 220px section list beside the scrolling content.

## Elevation & Depth

Flat by default. Depth comes from tonal steps (chrome, ground, surface, raised) and hairline borders. Hovering a card raises it one tonal step and strengthens its border; nothing lifts or casts a shadow. Shadows only appear on surfaces that float over the page.

### Shadow Vocabulary
- **Overlay** (`box-shadow: 0 24px 64px rgba(0,0,0,0.5)`): modals and the file-drop target, over a 60% black scrim.
- **Side sheet** (`box-shadow: -16px 0 48px rgba(0,0,0,0.45)`): the account sidebar sliding in from the right.

### Named Rules

**The Flat Page Rule.** Nothing on the page itself casts a shadow. A shadow means "this is floating over the page", and only modals and sheets float. No glass, no backdrop blur, no glow, no dot grid or texture behind content. The only image allowed behind the UI is an addon theme's background, shown at 30% opacity.

## Shapes

Corners scale from a single `--ui-radius` of 4px: inner chips and badges 6px, buttons, inputs and icon tiles 8px, cards, rail items, tables and the content frame 12px, empty-state panels 16px. Filter chips, accent swatches and status dots are fully round. Instance icons follow their container: 48px icons get 8px corners, the 72px header icon gets 12px.

Squared mode sets `--ui-radius` to 0 and forces `border-radius: 0` on every element, pseudo-elements included.

### Named Rules

**The Squared Test Rule.** Every component has to read correctly with all corners at zero. Structure comes from borders, tonal steps and spacing, never from roundness alone. If a shape only makes sense because it is rounded, it fails.

## Components

### Buttons
- **Shape:** gently rounded (8px), medium weight.
- **Play (solid):** accent fill, inverted text, filled triangle icon, wide padding (28px at xl on the Continue card, 24px at lg in the instance header, sm in the sticky strip). One per screen.
- **Play (quiet):** a 36px square tile with an accent icon on a 15% accent tint that fills solid on card hover or keyboard focus and scales to 95% when pressed. Used on every library card.
- **Stop:** solid Stop Red with a square icon, shown wherever Play would be while the game runs. In the compact sticky strip Close is a neutral soft button.
- **Soft / ghost:** neutral raised fill or no fill for secondary actions (Resume world, overflow menus, row actions).

### Chips
- **Filter chips:** 32px fully round pills in raised graphite with toned label text. The selected chip takes the accent fill with inverted text. They act as a radiogroup.
- **Source badges:** small 6px-cornered tags in raised graphite with muted text and an inset hairline. Modrinth, CurseForge and local files all look the same, with no brand colours.

### Cards / Containers
- **Corner Style:** 12px.
- **Background:** card surface at rest, raised graphite on hover, with the hairline strengthening to the strong hairline.
- **Shadow Strategy:** none (see Elevation).
- **Internal Padding:** 12px for instance cards, 16px for the Continue card, 14px for wide list cards.
- **Continue card:** a 64px icon, 16px semibold name with an accent "Now playing" badge while the game runs, a tabular meta line, an optional last-world line, then Resume (soft) and the screen's one solid Play.

### Inputs / Fields
- **Style:** soft fill in raised graphite with an 8px radius (6px at sm). The search in the titlebar sits at 70% raised and goes solid on hover and focus.
- **Focus:** accent ring and accent caret. Placeholders are dimmed.

### Navigation
- **Icon rail:** 40px square items with 12px corners, a 20px stroke icon at 1.8 weight, muted at rest, raised fill on hover, and a 15% accent fill with accent icon when active. Every item has a right-side tooltip. Recent instances show a 2px ring (accent when active) and an accent dot when the game is running. Addon buttons use the same rail variant.
- **Tabs:** 14px medium labels with 16px icons and a 2px bottom border that is accent on the active tab. Manage tabs collapse to icons in the compact sticky strip.
- **Segmented control:** the content-kind switch is a card-surface track with an inset hairline and 2px padding. The selected option is pressed graphite with highlighted text, and counts are dimmed tabular numbers. It shares a row with search, Add and an overflow menu.
- **Settings sections:** a vertical list of 8px-cornered rows, raised fill on the active row.

### Content Table
Calm and dense. It has a hairline frame with 12px corners, a card-surface header row with uppercase 11px column heads (the sorted column turns accent), 36px icons, names in medium weight, versions and dates in muted tabular text, and half-strength hairlines between rows. Disabled entries fade to 55%. The row hover is the card surface. The delete action is neutral until hovered, then turns red.

### Theme Swatch
A miniature launcher (chrome frame, rail dots, ground panel and card, with the accent in two places) drawn for each mode in the theme picker. It drops its own rounding for the Squared preview.

## Do's and Don'ts

### Do:
- **Do** keep each page on the shared gutter: max width 1400px, 32px sides, 28px top, 22px semibold page title.
- **Do** use the primary role for the accent so the user's pick and addon themes reach it.
- **Do** keep exactly one solid accent Play per screen and make every other Play quiet.
- **Do** keep destructive controls neutral at rest and red only on hover or confirm.
- **Do** use tabular figures for counts, versions, playtime and dates.
- **Do** check every new surface in Dark, OLED and Squared, with long German and Russian strings and with CJK text.
- **Do** leave an addon slot (rail, titlebar icon, header) where the old surface had one.

### Don't:
- **Don't** use glass, backdrop blur, glow, gradients behind content, or a dot grid.
- **Don't** use red, or red-adjacent hues, for anything except Stop and destructive actions.
- **Don't** colour source badges by provider brand.
- **Don't** use mono for UI copy.
- **Don't** put shadows on cards or other in-page surfaces.
- **Don't** hard-code grey hex values; OLED and addon tints have to be able to recolour them.
- **Don't** design a component whose meaning depends on rounded corners.

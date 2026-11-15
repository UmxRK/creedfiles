# Creed design

Date: 2026-11-15

## 1. Intent

Creed is a public, Arch-based Hyprland desktop distribution-of-dotfiles. Its
pitch: the customisability of end-4 / Caelestia, the ability to switch the
whole desktop between distinct OS "modes" (Creed original, Windows 10,
Windows 11, four macOS generations), and a hard memory budget an order of
magnitude below comparable shells.

It must look hand-crafted: restrained glow, artifact-free glass, deliberate
spacing and type. It is for many people on many machines, so it detects
hardware and adapts, and every system change it makes is reversible.

## 2. Goals

1. **Seven modes**, switchable live from the settings app or `creed mode <name>`:
   `creed` (original), `win10`, `win11`, `mac-tahoe`, `mac-sequoia`,
   `mac-bigsur`, `mac-aqua`. Each mode re-themes **everything**: shell, GTK/Qt
   apps, icons, cursors, fonts, terminal, prompt, browser, window decorations,
   lock screen, login screen, keybinds, window behaviour (float/tile).
2. **Per mode, recreate:** bar + dock/taskbar, launcher/Start/Spotlight,
   control center + notifications (popups and history), lock screen, SDDM login.
   - Windows 11 extras: Mica/Acrylic material, Snap Layouts, Widgets board,
     Alt-Tab + Task View. Windows 10 gets equivalents (Acrylic, tiled Start,
     Action Center).
   - macOS extras: global menu bar, dock magnification/bounce/running dots,
     Mission Control + Exposé + hot corners, Spotlight + Launchpad.
   - Tahoe Liquid Glass with editable refraction, thickness, tint, frost,
     clarity, specular highlight (optionally cursor-reactive), and
     per-element overrides (bar, dock, menus, notifications).
3. **Customisation** via both a GUI settings app and a single config file
   kept in sync, plus the `creed` CLI. Bar is heavily editable: position
   (top/bottom/left/right), style (floating, edge, islands, vertical), size,
   module order and presence.
4. **Motion presets**: Expressive (default for Creed mode), Smooth, Minimal,
   Off - each labelled with its relative GPU cost.
5. **Colours**: generated from wallpaper (matugen, run once on change) or a
   curated preset; user picks.
6. **Tweaks** in four categories - Gaming, Network/Wi-Fi, System/Memory,
   Battery/Laptop - auto-detected and offered at install with recommended
   items pre-ticked. Each tweak individually revertible; system files backed
   up; Btrfs/Snapper snapshot taken when available.
7. **Game Mode**: auto-detect (Steam/Lutris/Heroic/gamescope or fullscreen
   game window), hotkey and bar toggle; unloads non-essential shell surfaces
   (keeps a tiny overlay for volume/notifications), disables blur/glass/
   shadows/animations, restores everything on exit. MangoHud preset per mode.
8. **Hardware auto-configuration**: GPU vendor (NVIDIA/AMD/Intel/hybrid),
   CPU vendor/cores, laptop vs desktop, RAM size, monitors/refresh rates.
9. **Install/update**: TUI installer (gum) → first-login GTK4 welcome app;
   existing configs backed up to `~/.config/creed-backup/<date>` and restored
   by `creed uninstall`; updates via both an AUR package and `creed update`;
   user edits live in an override file never touched by updates.

## 3. Hard constraints

- **Memory budget: ≤ 60 MB PSS** for everything Creed adds, at idle, with
  every always-on surface drawn (bar, dock, wallpaper, notification daemon,
  any helpers) and all customisation enabled. Hyprland itself, the user's
  apps, and the settings app *while open* are excluded. If 60 MB proves
  impossible, the owner accepts ~120 MB as a ceiling - but the target is 60.
- **Scope of the gate (owner decision):** hard 60 MB gate on a single monitor
  up to 1440p. 4K and multi-monitor setups must stay as low as possible
  (static wallpaper uploaded once, single-buffered background surface) with a
  ceiling of ~120 MB. Hyprland's own memory (including the creed-glass and
  hyprbars plugins it loads) and GPU VRAM are reported by the harness but not
  counted. Shared shm buffers count toward Creed's PSS (proportional share).
- Measured honestly: PSS (not RSS) summed over all Creed processes, in a real
  Wayland session with surfaces mapped and rendered, after 10 minutes idle and
  after opening/closing every transient surface (to catch leaks). A test that
  hides UI to hit the number is a failure.
- Arch / Arch-based only (Arch, EndeavourOS, CachyOS, etc.).
- Public from day one: safe defaults, nothing destructive without consent,
  everything revertible.
- No proprietary assets: no Apple/Microsoft logos, icons, wallpapers, or
  fonts bundled. Free lookalikes (e.g. Inter, squircle icon mask,
  WhiteSur-style icons). Docs may explain how users install SF Pro themselves.

## 4. Non-goals

- Other distros (Fedora, Debian, NixOS).
- Pixel-perfect clones or trademarked branding.
- Re-implementing apps (file manager, browser) - Creed themes them.
- Global menus for apps that do not export menus (those show app name only).

## 5. Anti-goals (failure even if it "works")

- Generated-looking UI: over-glow, neon outlines, glass with banding/edge
  artifacts, off-centre elements, inconsistent radii/spacing.
- A RAM number achieved by hiding UI or measuring RSS of one process.
- Tweaks that silently break boot, networking, or suspend on some hardware.
- Mode switching that leaves stale state (wrong icons, half-themed apps).

## 6. Approaches considered

| | Approach | Est. PSS | Trade-off |
|---|---|---|---|
| **A (chosen)** | Custom Rust shell: one daemon, Wayland layer-shell via smithay-client-toolkit, own lightweight renderer | 15–30 MB | Lowest RAM, total control over motion/glass hooks; must build text input, scrolling, focus, a11y basics ourselves. |
| B | Rust + GTK4 + gtk4-layer-shell, one process | 40–65 MB | CSS theming speeds up 7 modes; GTK4 GL renderer pulls Mesa memory - borderline. |
| C | Assemble waybar/swaync/rofi/eww | >60 MB combined | Cannot do dock magnification, Launchpad, Mission Control properly. Rejected. |
| D | Quickshell (end-4/Caelestia stack) | 150 MB+ | Over budget by design. Rejected. |

Owner picked **A** directly (declined a measurement spike).

## 7. Architecture

```
┌──────────────────────── Hyprland ────────────────────────┐
│  creed-glass plugin (C++): refraction/frost/specular     │
│  shader on layer-surfaces by namespace; IPC-controlled   │
│  hyprbars plugin: per-mode titlebars                     │
└──────────────▲───────────────────────────▲───────────────┘
               │ hyprctl / plugin IPC       │ Hyprland IPC socket (events)
┌──────────────┴──────────┐   ┌────────────┴──────────────┐
│ creed-shell (Rust)      │◄──┤ creedd logic inside shell │
│ layer surfaces: bar,    │   │ (mode, game detect,       │
│ dock, popups, launcher, │   │ battery, tweaks status)   │
│ OSD, notifications      │   └───────────────────────────┘
│ D-Bus: notifications,   │
│ StatusNotifierItem tray,│   creed CLI (Rust, short-lived)
│ MPRIS, UPower, NM, BlueZ│   creed-settings (GTK4/libadwaita, on demand)
│ PipeWire (via wpctl/    │   installer (bash + gum)
│ pw native)              │   wallpaper: creed-shell background surface
└─────────────────────────┘   or swww (measured, chosen by budget)
```

### 7.1 creed-shell (Rust)

- Single process, single event loop (calloop). No Tokio runtime unless
  measurement shows it is cheap enough.
- Wayland: `smithay-client-toolkit` layer-shell surfaces.
- Rendering: CPU rasteriser (`tiny-skia`) into shm buffers by default;
  damage-tracked so idle frames cost nothing. A GL path (femtovg) is allowed
  only if measured within budget; glass/blur comes from the compositor
  plugin, so the shell itself never needs to sample the backdrop.
- Text: `cosmic-text` (shaping, fallback, emoji) with a bounded glyph cache.
- Icons: freedesktop icon lookup, SVG via `resvg`, rasterised on demand into
  a bounded LRU cache.
- Surfaces are created lazily and **destroyed** (not hidden) when closed;
  their memory is released (verify with `malloc_trim`/allocator choice).
- Widget model: small retained tree with layout (flex-like rows/columns,
  padding, alignment), animated properties (spring + cubic-bezier easing),
  and hit-testing. Modes are **data**, not code where possible.
- System integrations over D-Bus (`zbus`, blocking/async-io executor, no
  Tokio): org.freedesktop.Notifications server, StatusNotifierWatcher/Host,
  MPRIS, UPower, NetworkManager, BlueZ, logind (lock/sleep), PipeWire volume.
- Global menu: com.canonical.dbusmenu / AppMenu registrar for apps that
  export menus; otherwise app name.

### 7.2 Mode definitions

`modes/<name>/` contains:
- `mode.toml` - metadata, description, preview image, defaults
  (float/tile, keybind profile, motion preset, glass profile).
- `layout.toml` - which surfaces exist and their widget trees/positions.
- `style.toml` - tokens: colours (or "from-palette"), radii, spacing, fonts,
  shadows, material (glass/mica/acrylic/flat) per element.
- `hypr.conf` - Hyprland snippet (decoration, animations, window rules,
  keybinds) sourced by the mode switcher.
- `apps/` - templates for GTK/Qt/kitty/foot/starship/browser/MangoHud/
  hyprlock/SDDM rendered by `creed mode` with the current palette.

Mode switch = render templates → atomically swap symlinks in
`~/.config/creed/active` → `hyprctl reload` → shell hot-reloads mode →
`gsettings`/`qt6ct`/cursor/icon updates → notify running apps
(GTK via xsettingsd-equivalent/portal settings, terminals via OSC/reload).

### 7.3 Config

- `~/.config/creed/creed.toml` - single user config (generated with comments).
- `~/.config/creed/overrides.toml` - user edits; never touched by updates.
- Schema-versioned; `creed update` runs migrations.
- Settings app reads/writes the same files; shell watches with inotify and
  hot-reloads. Invalid config → shell keeps last good config and shows a
  notification pointing at the line.

### 7.4 creed-glass Hyprland plugin

- Applies to layer-surfaces matched by namespace (e.g. `creed-bar`,
  `creed-dock`) and optionally to windows by rule.
- Parameters: refraction strength, edge thickness, tint (rgba), frost (blur
  radius), clarity, specular intensity/angle, cursor-reactive toggle.
  Per-namespace overrides.
- Quality tiers Low/Medium/High/Ultra; installer picks by GPU; weak/integrated
  GPUs default to frost-only (Hyprland's native blur) while keeping the look.
- Built and kept in sync with Hyprland via `hyprpm`. If the plugin fails to
  load (Hyprland ABI change), shell falls back to native blur and notifies.
- Mica/Acrylic for Windows modes are profiles of the same plugin
  (wallpaper-tinted heavy blur + noise), not separate code.

### 7.5 Tweaks

`tweaks/<category>/<name>/` with `tweak.toml` (title, description, what files
/ units it touches, detection predicate, risk level, recommended-for) and
`apply`/`revert`/`status` scripts. Engine (`creed tweak`) records state in
`/var/lib/creed/tweaks.json`, backs up every touched file first, takes a
Snapper/Btrfs snapshot before a batch when available. Privileged ops go
through `pkexec`/sudo with a clear prompt.

**Explain before acting (owner rule, applies to every tweak, auto or manual):**
before anything is applied, Creed shows a plain-language summary of exactly
what will happen - files/settings changed, packages or caches removed (with
sizes and counts, e.g. "Removes 3.2 GB: 412 old package versions, 18 orphan
packages, Steam shader cache"), apps that will be closed, services toggled,
whether it is revertible - and waits for confirmation. "Automatic" tweaks
(e.g. Game Mode, battery profile switching) are explained and consented to
once at install or when first enabled; after that they act silently but are
listed in `creed tweak status` and the settings app. Destructive cleanup
always asks each time.

Initial catalogue (each opt-in at install, recommended ones pre-ticked by
detection):
- **Gaming**: GPU env vars (NVIDIA: `LIBVA_DRIVER_NAME`, `__GLX_VENDOR_LIBRARY_NAME`,
  `NVD_BACKEND`, etc.; hybrid: prime offload wrapper), gamemode, gamescope,
  VRR/`allow_tearing` + immediate render for fullscreen games, CPU governor
  switch in Game Mode, `vm.max_map_count`, MangoHud presets, ntsync if kernel
  supports it.
- **Network**: Wi-Fi power-save off (NetworkManager), BBR + fq, optional
  DNS-over-TLS via systemd-resolved, IPv6 privacy, regulatory domain.
- **System/Memory**: zram-generator sizing by RAM, swappiness/vfs_cache_pressure,
  I/O scheduler per device type, journald size cap, systemd-oomd, faster boot
  (disable wait-online etc. if safe), optional service cleanup list.
- **Cleanup / Free space** (owner request): clear pacman/paru cache
  (`paccache`), remove orphan packages, trim journal, clear app caches
  (thumbnails, browser/Electron/Steam shader caches, `~/.cache` by app),
  close/unload idle background apps to free RAM. These are **never automatic**
  and are not reversible by nature, so they require a dry-run preview first.
- **Battery/Laptop**: power-profiles-daemon (or TLP, not both), auto
  profile on AC/battery, lower refresh rate + Minimal motion + glass Low on
  battery, USB autosuspend.

### 7.6 Game Mode

Shell watches Hyprland events for windows matching a game ruleset
(class/initialTitle from Steam `steam_app_*`, gamescope, Lutris/Heroic
children, or fullscreen + known game process) and the hotkey/bar toggle.
On enter: `hyprctl --batch` disable blur/shadows/animations/glass, enable
tearing rule for that window, destroy non-essential surfaces (keep OSD +
notification popups), set CPU governor via gamemode, pause wallpaper
animation. On exit: restore from a saved state snapshot.

### 7.7 Installer & welcome

- `install.sh` (bash + gum): preflight (Arch check, not root, network,
  Hyprland version), hardware report, backup existing configs, install
  packages (pacman + paru/yay bootstrap), tweak checklists per category with
  detection-based pre-ticks, build plugins via hyprpm, enable services, SDDM
  theme, summary. Idempotent; resumable; `--dry-run`; logs to
  `~/.cache/creed/install.log`.
- First login: `creed-settings --welcome` (GTK4/libadwaita) - mode picker
  with previews and plain-language descriptions, wallpaper/colours, motion,
  glass, bar layout. Same app later serves as the settings app.
- Updates: AUR package `creed-dotfiles` (+ `creed-glass-git` plugin via
  hyprpm) and `creed update` for git installs.
- `creed uninstall` restores backups and reverts all tweaks.

### 7.8 Default apps themed

foot (default) and kitty; Thunar (default) and Nautilus; zsh or fish +
starship; Firefox (userChrome) and Chromium (theme colours) palette sync;
MangoHud.

## 8. Phasing

The full scope is several projects. Each phase ships finished and tested.

1. **Phase 1 - Core + Creed mode.** Shell engine, widget/layout/animation
   system, all D-Bus integrations, creed-glass plugin (frost + refraction),
   config + hot reload, `creed` CLI, mode system (with only `creed` mode),
   tweak engine + full tweak catalogue, Game Mode, installer, welcome/settings
   app, hyprlock + SDDM theme for Creed, RAM test harness and CI.
2. Phase 2 - Windows 11 mode.
3. Phase 3 - Windows 10 mode.
4. Phase 4 - macOS Tahoe (Liquid Glass showcase).
5. Phase 5 - macOS Sequoia/Sonoma, Big Sur, Classic Aqua.

Order of 2–5 may be changed by the owner.

### 8.1 Creed mode (Phase 1 look)

Soft & playful: rounded pills, warm palette, generous radii, expressive
spring motion by default. Default bar: top, floating, rounded, gap from
edges. Colours: wallpaper-generated or curated preset (owner toggles).
Restrained glow - shadows soft and low-opacity, no neon outlines.

Creed mode components in Phase 1 (owner decision): top bar, **dock**,
launcher, control center, notifications (popups + history), **workspace
overview** (exposé-style with live window thumbnails), **widgets dashboard**
(calendar, media, weather, system stats), **OSDs** (volume/brightness/etc.),
**clipboard history**, **emoji/glyph picker**, hyprlock lock screen, SDDM
login theme.

Engine primitives built in Phase 1 so later modes need no rewrite: icon
magnification and bounce, window thumbnails (hyprland-toplevel-export /
screencopy), full-screen overlays (overview, Launchpad, Mission Control,
Task View), grid snapping overlays (Snap Layouts), slide-out panels
(Widgets board), global-menu rendering.

### 8.2 Accessibility (Phase 1, owner decision: everything)

- Full keyboard navigation of every surface with visible focus rings.
- Input methods (fcitx5/IBus via text-input-v3 + input-method protocols) in
  every text field, so CJK typing works in the launcher and pickers.
- HiDPI / fractional scaling (wp-fractional-scale-v1), large-text and
  high-contrast options, reduced-motion respected.
- **Screen reader support from day one**: the widget tree exposes an
  accessibility tree via AccessKit (`accesskit_unix` → AT-SPI) so Orca can
  read every surface. Roles, names, states, and focus changes are part of
  each widget's definition, not bolted on.

## 9. Testing & verification

- **Unit tests** (Rust): layout, config parsing/migration, animation curves,
  game detection rules, tweak engine state machine.
- **Headless integration**: run Hyprland headless (or a wlroots headless
  compositor where Hyprland headless is unavailable) in CI/container; start
  creed-shell; screenshot every surface in every mode and motion preset with
  `grim`; compare to golden images; automated geometry checks (element
  centring, alignment, no clipping, equal padding) from the shell's own
  layout dump (`creed debug layout --json`).
- **RAM gate**: script sums PSS (`/proc/<pid>/smaps_rollup`) of all Creed
  processes with all always-on surfaces mapped, after cycling every
  transient surface 20×, after 10 min idle. CI fails above 60 MB.
- **Installer tests**: run in an Arch container with `--dry-run` and real
  install; verify idempotency and full `creed uninstall` round-trip (byte-for-
  byte restoration of backed-up files).
- **Tweak tests**: each tweak's apply → status → revert verified in a
  container/VM; revert restores originals exactly.
- **Config validation**: `Hyprland --verify-config` on every mode's generated
  config.
- **Limit of verification**: the build container has no real GPU. Visual
  polish, glass shader quality and real-game behaviour on NVIDIA/AMD/Intel
  require owner testing on hardware; a checklist is provided per phase.

## 10. Left to the builder

- Exact crate versions; allocator choice (system vs mimalloc) by measurement.
- Wallpaper: own background surface vs swww, whichever fits budget.
- Exact game-detection ruleset contents.
- Icon theme and font picks for Creed mode within the "soft & playful" brief
  (owner reviews screenshots).

## 11. Builder calls (made without owner input)

- Arch helper bootstrap: paru, falling back to existing yay.
- Default terminal foot, default file manager Thunar (both swappable).
- Config format TOML.
- CPU raster rendering by default to protect the memory budget.
- **Hyprland support**: latest stable release only; installer refuses older
  versions with a clear message. CI tracks stable; -git is best-effort.
- **Plugin delivery**: creed-glass and hyprbars are built per user via
  `hyprpm` (run by `creed setup` and on Hyprland updates). The AUR package
  does not build plugins.
- **Install routes**: git `install.sh` is primary. The AUR package installs
  files to `/usr/share/creed` and binaries to `/usr/bin`; the user then runs
  `creed setup` (same per-user wizard: backups, tweaks, plugins). `creed
  update` detects an AUR install and defers to the package manager.
- **Lock screen**: hyprlock with a config generated per mode.
- **Login screen**: SDDM theme fixed to Creed by default; an opt-in
  "login screen follows mode" installs a polkit rule once so mode switches
  can update it without a password each time.
- **Licence**: GPL-3.0 (allows bundling GPL icon/GTK themes).

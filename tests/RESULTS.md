# Verification results

Measured in an Arch Linux root (Hyprland 0.57.2 for config validation; headless sway with the pixman renderer for live shell tests, including the downloaded look-alike assets). Memory is PSS of all Creed processes with every always-on surface drawn, after opening/closing every panel 20 times (200 opens) and idling; "idle CPU" is scheduler ticks used during 3 idle seconds.

```
creed 1080p: PSS start: 20 MB   PSS after cycling: 28 MB   limit: 60 MB   idle CPU: 0 ticks/3s PASS
win11 1080p: PSS start: 20 MB   PSS after cycling: 27 MB   limit: 60 MB   idle CPU: 0 ticks/3s PASS
win10 1080p: PSS start: 19 MB   PSS after cycling: 28 MB   limit: 60 MB   idle CPU: 0 ticks/3s PASS
mac-tahoe 1080p: PSS start: 25 MB   PSS after cycling: 33 MB   limit: 60 MB   idle CPU: 1 ticks/3s PASS
mac-sequoia 1080p: PSS start: 23 MB   PSS after cycling: 30 MB   limit: 60 MB   idle CPU: 0 ticks/3s PASS
mac-bigsur 1080p: PSS start: 23 MB   PSS after cycling: 31 MB   limit: 60 MB   idle CPU: 0 ticks/3s PASS
mac-aqua 1080p: PSS start: 22 MB   PSS after cycling: 30 MB   limit: 60 MB   idle CPU: 1 ticks/3s PASS
creed 4K: PSS start: 32 MB   PSS after cycling: 40 MB   limit: 120 MB   idle CPU: 0 ticks/3s PASS
mac-tahoe 4K: PSS start: 37 MB   PSS after cycling: 45 MB   limit: 120 MB   idle CPU: 0 ticks/3s PASS
win11 4K: PSS start: 32 MB   PSS after cycling: 40 MB   limit: 120 MB   idle CPU: 0 ticks/3s PASS
```

`tests/run-all.sh` passed every stage:
- lint: clippy with warnings as errors, shellcheck (53 scripts), pyflakes
- unit tests (creed-core 33, creed-shell 17)
- every mode × glass quality / motion / colour scheme accepted by `Hyprland --verify-config`
- generated foot, starship, kitty, Lua, CSS, Qt, MangoHud and hyprlock files for every mode in light and dark, checked by the apps' own parsers
- CLI: every command, bad input (non-zero exit, a message, never a panic), config round trips
- Creed Settings: every page opens without exceptions, edits of every type persist, out-of-range edits are rejected, welcome flow
- screen reader: every control reaches AT-SPI with a name in all seven modes
- SDDM: the three login themes render with the real greeter, no QML errors
- every shell dispatcher accepted by Hyprland
- tweaks: apply → re-apply → status → revert with byte-identical restore; cleanups delete only caches
- polkit rule: grants only local, active admins running the helper
- `creed setup` → `creed uninstall` restores a user's original files byte-for-byte
- the memory gate above
- screenshots of every surface in every mode, with the layout geometry check

Also checked: `makepkg` builds the package (running the unit tests in `check()`) and `namcap` reports no problems with the PKGBUILD.

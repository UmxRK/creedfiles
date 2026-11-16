# Verification results

Measured in an Arch Linux root (Hyprland 0.57.2 for config validation; headless sway with the pixman renderer for live shell tests). Memory is PSS of all Creed processes with every always-on surface drawn, after opening/closing every panel 20× (200 opens) and idling; "idle CPU" is scheduler ticks used during 3 idle seconds.

```
creed 1440p: PSS start: 26 MB   PSS after cycling: 32 MB   limit: 60 MB   idle CPU: 1 ticks/3s PASS
win11 1440p: PSS start: 22 MB   PSS after cycling: 28 MB   limit: 60 MB   idle CPU: 1 ticks/3s PASS
win10 1440p: PSS start: 22 MB   PSS after cycling: 28 MB   limit: 60 MB   idle CPU: 0 ticks/3s PASS
mac-tahoe 1440p: PSS start: 25 MB   PSS after cycling: 31 MB   limit: 60 MB   idle CPU: 0 ticks/3s PASS
mac-sequoia 1440p: PSS start: 25 MB   PSS after cycling: 31 MB   limit: 60 MB   idle CPU: 0 ticks/3s PASS
mac-bigsur 1440p: PSS start: 25 MB   PSS after cycling: 31 MB   limit: 60 MB   idle CPU: 0 ticks/3s PASS
mac-aqua 1440p: PSS start: 26 MB   PSS after cycling: 31 MB   limit: 60 MB   idle CPU: 0 ticks/3s PASS
creed 4K: PSS start: 50 MB   PSS after cycling: 42 MB   limit: 120 MB   idle CPU: 1 ticks/3s PASS
win11 4K: PSS start: 47 MB   PSS after cycling: 37 MB   limit: 120 MB   idle CPU: 0 ticks/3s PASS
mac-tahoe 4K: PSS start: 35 MB   PSS after cycling: 41 MB   limit: 120 MB   idle CPU: 0 ticks/3s PASS
creed 1080p: PSS start: 22 MB   PSS after cycling: 28 MB   limit: 60 MB   idle CPU: 0 ticks/3s PASS
win11 1080p: PSS start: 20 MB   PSS after cycling: 24 MB   limit: 60 MB   idle CPU: 0 ticks/3s PASS
win10 1080p: PSS start: 19 MB   PSS after cycling: 25 MB   limit: 60 MB   idle CPU: 0 ticks/3s PASS
mac-tahoe 1080p: PSS start: 23 MB   PSS after cycling: 28 MB   limit: 60 MB   idle CPU: 0 ticks/3s PASS
mac-sequoia 1080p: PSS start: 22 MB   PSS after cycling: 28 MB   limit: 60 MB   idle CPU: 0 ticks/3s PASS
mac-bigsur 1080p: PSS start: 22 MB   PSS after cycling: 28 MB   limit: 60 MB   idle CPU: 0 ticks/3s PASS
mac-aqua 1080p: PSS start: 22 MB   PSS after cycling: 28 MB   limit: 60 MB   idle CPU: 1 ticks/3s PASS
✓ layout geometry ok (77 dumps)
```

Also passing: unit tests (creed-core, creed-shell); every mode × glass quality / motion / colour-scheme variant validated by `Hyprland --verify-config`; all 19 reversible tweaks apply → re-apply → status → revert with byte-identical restore; destructive cleanups delete only caches; `creed setup` → `creed uninstall` restores a user's original files byte-for-byte; root helper rejects tampered or unknown input.

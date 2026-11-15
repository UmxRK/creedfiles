//! Names of bar modules the shell knows how to draw.

pub const BAR_MODULES: &[&str] = &[
    "launcher",     // Creed button / Start button / Apple-style menu
    "workspaces",   // workspace pills
    "window",       // focused window title
    "appmenu",      // macOS-style app name + menus
    "taskbar",      // running/pinned apps (Windows modes)
    "search",       // Windows 11 search box
    "taskview",     // Task View / overview button
    "widgets",      // Windows widgets button
    "clock",        // time (and date in Windows modes)
    "date",
    "tray",         // StatusNotifierItem tray
    "media",        // MPRIS now playing
    "network",
    "bluetooth",
    "volume",
    "battery",
    "sysstat",      // cpu/ram
    "weather",
    "keyboard",     // keyboard layout
    "gamemode",     // Game Mode toggle
    "notifications",// notification bell / count
    "control",      // control center toggle
    "power",        // power menu
    "spacer",
];

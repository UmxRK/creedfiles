-- Creed Hyprland entry point (managed by Creed; safe to read, not to edit).
-- Your own settings: ~/.config/hypr/user.lua - loaded last, never overwritten.
local home = os.getenv("HOME")
local generated = home .. "/.config/creed/generated/hyprland.lua"
local user = home .. "/.config/hypr/user.lua"

local function exists(path)
    local f = io.open(path, "r")
    if f then f:close() return true end
    return false
end

if exists(generated) then
    dofile(generated)
else
    -- Creed has not generated a config yet: minimal safe fallback with a terminal bind.
    hl.bind("SUPER + Return", hl.dsp.exec_cmd("foot"))
    hl.on("hyprland.start", function() hl.exec_cmd("creed theme apply --no-reload && hyprctl reload") end)
end

-- Tweaks that adjust Hyprland itself (e.g. raw mouse input) drop small files here.
for _, name in ipairs({ "raw-mouse" }) do
    local t = home .. "/.config/hypr/creed-tweaks/" .. name .. ".lua"
    if exists(t) then dofile(t) end
end

if exists(user) then
    dofile(user)
end

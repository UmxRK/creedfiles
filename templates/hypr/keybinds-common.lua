------------------------------ shared keys ------------------------------
-- Media / hardware keys work in every mode and on the lock screen.
hl.bind("XF86AudioRaiseVolume", hl.dsp.exec_cmd("wpctl set-volume -l 1 @DEFAULT_AUDIO_SINK@ 5%+"), { locked = true, repeating = true })
hl.bind("XF86AudioLowerVolume", hl.dsp.exec_cmd("wpctl set-volume @DEFAULT_AUDIO_SINK@ 5%-"), { locked = true, repeating = true })
hl.bind("XF86AudioMute", hl.dsp.exec_cmd("wpctl set-mute @DEFAULT_AUDIO_SINK@ toggle"), { locked = true })
hl.bind("XF86AudioMicMute", hl.dsp.exec_cmd("wpctl set-mute @DEFAULT_AUDIO_SOURCE@ toggle"), { locked = true })
hl.bind("XF86MonBrightnessUp", hl.dsp.exec_cmd("brightnessctl -q set 5%+ && creed osd brightness"), { locked = true, repeating = true })
hl.bind("XF86MonBrightnessDown", hl.dsp.exec_cmd("brightnessctl -q set 5%- && creed osd brightness"), { locked = true, repeating = true })
hl.bind("XF86AudioPlay", hl.dsp.exec_cmd("playerctl play-pause"), { locked = true })
hl.bind("XF86AudioPause", hl.dsp.exec_cmd("playerctl play-pause"), { locked = true })
hl.bind("XF86AudioNext", hl.dsp.exec_cmd("playerctl next"), { locked = true })
hl.bind("XF86AudioPrev", hl.dsp.exec_cmd("playerctl previous"), { locked = true })
-- Screenshots (region to clipboard, full screen to ~/Pictures/Screenshots).
hl.bind("Print", hl.dsp.exec_cmd("creed screenshot region"))
hl.bind("SHIFT + Print", hl.dsp.exec_cmd("creed screenshot screen"))
-- Mouse: move / resize with the main modifier.
hl.bind("SUPER + mouse:272", hl.dsp.window.drag(), { mouse = true })
hl.bind("SUPER + mouse:273", hl.dsp.window.resize(), { mouse = true })
-- Game Mode and the mode picker are always available.
hl.bind("SUPER + G", hl.dsp.exec_cmd("creed game toggle"), { description = "Toggle Game Mode" })
hl.bind("SUPER + CTRL + M", hl.dsp.exec_cmd("creed-settings --page modes"), { description = "Change mode" })
hl.bind("SUPER + Z", hl.dsp.exec_cmd("creed toggle snap"), { description = "Snap layouts" })
hl.bind("SUPER + V", hl.dsp.exec_cmd("creed toggle clipboard"), { description = "Clipboard history" })
hl.bind("SUPER + period", hl.dsp.exec_cmd("creed toggle emoji"), { description = "Emoji picker" })
for i = 1, 9 do
    hl.bind("SUPER + " .. i, hl.dsp.focus({ workspace = i }))
    hl.bind("SUPER + SHIFT + " .. i, hl.dsp.window.move({ workspace = i }))
end

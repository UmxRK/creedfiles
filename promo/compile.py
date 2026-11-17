"""Cuts the promo clips into three ~54 s compilations in Blender's Video Sequence Editor.
blender -b --python promo/compile.py -- <1|2|3> [preview t,t,...]
Features: a 1.5 s cold-open teaser, a four-part story bar, #4 -> #1 countdown, hard cuts on the beat
with a punch-in zoom and flash, an original music bed with risers/impacts, a comment call to action,
a "Part N of 3" tease, and a crossfade back to the first frame so the video loops seamlessly."""
import os
import subprocess
import sys

import bpy

A = sys.argv[sys.argv.index('--') + 1:]
PART = int(A[0])
ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
VID = ROOT + '/promo/videos/%s.mp4'
OUT = os.environ.get('PROMO_OUT', ROOT + '/promo/out')
FPS, W, H = 30, 1080, 1920
FD = '/usr/share/fonts/opentype/inter/InterDisplay-%s.otf'

# (title, chapters (#4..#1) as (clip, seconds used), end card (clip, from s), teaser (clip, s) x5, CTA)
PARTS = {
    1: ('Any era. Any look.', [('command', 12.0), ('aqua', 12.0), ('era', 12.0), ('seven', 12.0)], ('seven', 12.55),
        [('seven', 4.2), ('aqua', 6.0), ('seven', 7.3), ('command', 9.0), ('era', 11.4)],
        'Which look would you run?'),
    2: ('Built to be light.', [('tweaks', 12.0), ('game', 12.0), ('ram', 12.0), ('stats', 12.0)], ('stats', 12.3),
        [('ram', 5.5), ('stats', 3.6), ('game', 8.8), ('tweaks', 7.5), ('ram', 1.3)],
        'Guess your desktop\'s RAM use.'),
    3: ('It\'s the details.', [('details', 12.0), ('win11', 12.0), ('material', 12.0), ('glass', 12.0)], ('details', 12.75),
        [('glass', 6.5), ('win11', 3.5), ('material', 3.8), ('details', 6.0), ('glass', 3.5)],
        'Which detail got you?'),
}
TITLE, CHAPTERS, END, TEASE, CTA = PARTS[PART]
SKIP, END_SKIP = .3, .35                   # clip seconds skipped at chapter / end-card starts
TEASE_F, FLASH = 9, 5                      # 5 flashes x 9 frames = 1.5 s (3 beats at 120 BPM)

bpy.ops.wm.read_factory_settings(use_empty=True)
S = bpy.context.scene
S.render.resolution_x, S.render.resolution_y, S.render.fps = W, H, FPS
S.render.resolution_percentage = 100
S.view_settings.view_transform = 'Standard'
ed = S.sequence_editor_create()
SEQ = ed.sequences
fonts = {}
ch = [1]


def nextch():
    ch[0] += 1
    return ch[0]


def key(strip, path, frames_vals, interp='LINEAR', easing='EASE_OUT'):
    """Keyframes strip.<path> (e.g. 'transform.scale_x', 'blend_alpha'); sets interpolation on them."""
    obj, attr = strip, path
    if '.' in path:
        sub, attr = path.split('.')
        obj = getattr(strip, sub)
    for f, v in frames_vals:
        setattr(obj, attr, v)
        obj.keyframe_insert(attr, frame=f)
    act = S.animation_data.action
    dp = f'sequence_editor.sequences_all["{strip.name}"].{path}'
    for fc in act.fcurves:
        if fc.data_path == dp:
            for p in fc.keyframe_points:
                p.interpolation = interp
                p.easing = easing


def movie(name, clip, at, src_s, frames):
    s = SEQ.new_movie(name, VID % clip, nextch(), at - int(round(src_s * FPS)))
    s.frame_offset_start = int(round(src_s * FPS))
    s.frame_final_duration = frames
    s.frame_start = at - s.frame_offset_start
    return s


def color(name, rgb, at, frames, alpha=1.0):
    s = SEQ.new_effect(name, 'COLOR', nextch(), frame_start=at, frame_end=at + frames)
    s.color = rgb
    s.blend_type = 'ALPHA_OVER'
    s.blend_alpha = alpha
    return s


def rect(name, rgb, at, frames, x, y, w, h, alpha=1.0):
    """Colour strip shaped into a rectangle; x, y are the top-left corner in pixels from the top-left."""
    s = color(name, rgb, at, frames, alpha)
    s.transform.scale_x, s.transform.scale_y = w / W, h / H
    s.transform.offset_x, s.transform.offset_y = x + w / 2 - W / 2, H / 2 - (y + h / 2)
    return s


def text(name, body, at, frames, y, size, weight='Bold', col=(1, 1, 1, 1), box=False, pop=True, x=.5, align='CENTER'):
    """Text strip; y is the vertical centre as a fraction from the TOP of the frame."""
    path = FD % weight
    if path not in fonts:
        fonts[path] = bpy.data.fonts.load(path)
    s = SEQ.new_effect(name, 'TEXT', nextch(), frame_start=at, frame_end=at + frames)
    s.text, s.font, s.font_size, s.color = body, fonts[path], size, col
    s.location = (x, 1 - y)
    s.align_x, s.align_y = align, 'CENTER'
    s.use_shadow, s.shadow_color = True, (0, 0, 0, .55)
    if box:
        s.use_box, s.box_color, s.box_margin = True, (0, 0, 0, .55), .012
    s.blend_type = 'ALPHA_OVER'
    if pop:
        s.transform.origin = (x, 1 - y)
        key(s, 'transform.scale_x', [(at, .6), (at + 7, 1.0)], 'BACK')
        key(s, 'transform.scale_y', [(at, .6), (at + 7, 1.0)], 'BACK')
        key(s, 'blend_alpha', [(at, 0.0), (at + 4, 1.0)], 'SINE')
    return s


# ── timeline ──
t = 1
cuts = []                                   # seconds: where the beat-synced cuts land
# Cold open: five quick flashes of what is coming, under a "watch till #1" hook.
for i, (clip, src) in enumerate(TEASE):
    m = movie(f'tease{i}', clip, t + i * TEASE_F, src, TEASE_F)
    key(m, 'transform.scale_x', [(m.frame_final_start, 1.12), (m.frame_final_end - 1, 1.0)], 'SINE')
    key(m, 'transform.scale_y', [(m.frame_final_start, 1.12), (m.frame_final_end - 1, 1.0)], 'SINE')
teaser_end = t + FLASH * TEASE_F
text('hook', TITLE, t, teaser_end - t, .46, 96, 'ExtraBold', box=True)
text('hook2', 'Watch till #1', t + 6, teaser_end - t - 6, .535, 52, 'SemiBold', col=(1, .85, .45, 1), box=True)
t = teaser_end
chapter_spans = []
for i, (clip, secs) in enumerate(CHAPTERS):
    n = int(round(secs * FPS))
    m = movie(f'ch{i}', clip, t, SKIP, n)        # skip the first word's pop-in so the cut lands on content
    # Punch-in on the cut.
    key(m, 'transform.scale_x', [(t, 1.08), (t + 8, 1.0)], 'EXPO')
    key(m, 'transform.scale_y', [(t, 1.08), (t + 8, 1.0)], 'EXPO')
    fl = color(f'flash{i}', (1, 1, 1), t, 6, .0)
    key(fl, 'blend_alpha', [(t, .5), (t + 5, 0.0)], 'SINE')
    cuts.append((t - 1) / FPS)
    chapter_spans.append((t, t + n))
    text(f'rank{i}', f'#{4 - i}', t, n, .088, 46, 'ExtraBold', x=.075, align='LEFT')
    t += n
# End card: the closing title from one clip, plus the call to action.
end_clip, end_from = END
end_from += END_SKIP                       # land on the logo already in place
n_end = int(round((15.0 - end_from) * FPS))
movie('end', 'end-' + end_clip, t, 0.0, n_end)   # same outro re-rendered without the repo link (PROMO_LINK=0)
cuts.append((t - 1) / FPS)
text('cta', CTA, t + 2, n_end - 2, .715, 54, 'Bold', box=True)
nxt = {1: 'Part 1 of 3  ·  follow for Part 2', 2: 'Part 2 of 3  ·  follow for Part 3', 3: 'Part 3 of 3  ·  start again at Part 1'}[PART]
text('next', nxt, t + 8, n_end - 8, .77, 36, 'SemiBold', col=(.85, .85, .9, 1))
t += n_end
# Seamless loop: fade into the very first frame of the video.
LOOP = 12
lp = movie('loop', TEASE[0][0], t - LOOP, TEASE[0][1], LOOP)
key(lp, 'transform.scale_x', [(t - LOOP, 1.12)])
key(lp, 'transform.scale_y', [(t - LOOP, 1.12)])
key(lp, 'blend_alpha', [(t - LOOP, 0.0), (t - 1, 1.0)], 'SINE')
lp.blend_type = 'ALPHA_OVER'
end = t - 1
S.frame_start, S.frame_end = 1, end

# Story bar: four segments at the top, each filling during its chapter (and the countdown above).
BX, BY, BW, BH, GAP = 60, 118, (W - 120 - 3 * 12) / 4, 7, 12
for i, (a, b) in enumerate(chapter_spans):
    x = BX + i * (BW + GAP)
    rect(f'segbg{i}', (1, 1, 1), 1, end, x, BY, BW, BH, .3)
    f = rect(f'seg{i}', (1, 1, 1), a, end - a + 1, x, BY, BW, BH, .95)
    key(f, 'transform.scale_x', [(a, 0.0), (b, BW / W)])
    key(f, 'transform.offset_x', [(a, x - W / 2), (b, x + BW / 2 - W / 2)])

# Music: original, beat-synced (120 BPM; every cut is on a beat).
os.makedirs(OUT, exist_ok=True)
raw, wav = f'{OUT}/music{PART}.raw.wav', f'{OUT}/music{PART}.wav'
subprocess.run([sys.executable if os.path.basename(sys.executable).startswith('python') else 'python3', ROOT + '/promo/music.py', raw,
                str(end / FPS + .05), ','.join(f'{c:.3f}' for c in cuts)], check=True)
subprocess.run([ROOT + '/promo/loudnorm.sh', raw, wav], check=True)
snd = SEQ.new_sound('music', wav, nextch(), 1)
snd.frame_final_end = end + 1

r = S.render
if len(A) > 1 and A[1] == 'preview':
    r.image_settings.file_format = 'JPEG'
    for tt in A[2].split(','):
        S.frame_set(int(round(float(tt) * FPS)) + 1)
        r.filepath = f'{OUT}/part{PART}_{float(tt):05.2f}.jpg'
        bpy.ops.render.render(write_still=True)
else:
    r.image_settings.file_format = 'FFMPEG'
    r.ffmpeg.format, r.ffmpeg.codec = 'MPEG4', 'H264'
    r.ffmpeg.constant_rate_factor, r.ffmpeg.ffmpeg_preset = 'PERC_LOSSLESS', 'GOOD'
    r.ffmpeg.audio_codec, r.ffmpeg.audio_bitrate, r.ffmpeg.audio_mixrate, r.ffmpeg.audio_channels = 'AAC', 256, 48000, 'STEREO'
    r.filepath = f'{OUT}/part{PART}_master.mp4'
    bpy.ops.render.render(animation=True)
    print('CUTS', ','.join(f'{c:.2f}' for c in cuts), 'END', end)

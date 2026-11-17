"""Creed promo videos, built and rendered in Blender (EEVEE).
blender -b --python promo/gen.py -- <video> still <t,t,...> | anim   (frames -> $OUT/<video>/)
Screens are real 2x captures from tests/out/<mode>-3840x2160 (tests/visual.sh <mode> 3840x2160 2)."""
import bpy, bmesh, math, os, sys
from mathutils import Vector
A = sys.argv[sys.argv.index('--') + 1:]
NAME, KIND = A[0], A[1]
FPS, DUR = 30, 15.0
ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
OUT = os.environ.get('PROMO_OUT', ROOT + '/promo/out')
SH = ROOT + '/tests/out/%s-3840x2160/%s.png'
FD = '/usr/share/fonts/opentype/inter/InterDisplay-%s.otf'
MONO = os.environ.get('PROMO_MONO', '/opt/arch/root.x86_64/usr/share/fonts/TTF/JetBrainsMono-Bold.ttf')
F = lambda t: int(math.floor(t * FPS + 0.5 + 1e-6)) + 1
W = (1, 1, 1)

bpy.ops.wm.read_factory_settings(use_empty=True)
S = bpy.context.scene
S.render.engine = 'BLENDER_EEVEE_NEXT'; S.eevee.taa_render_samples = 4; S.eevee.use_shadows = False
S.render.resolution_x, S.render.resolution_y, S.render.fps = 1080, 1920, FPS
S.frame_start, S.frame_end = 1, int(DUR * FPS)
S.view_settings.view_transform = 'Standard'
S.world = bpy.data.worlds.new('w'); S.world.use_nodes = True
S.world.node_tree.nodes['Background'].inputs[0].default_value = (0, 0, 0, 1)
COL = S.collection
fonts, mats, dyn = {}, {}, []

def link(o, parent=None):
    COL.objects.link(o)
    if parent: o.parent = parent
    return o

def empty(loc=(0, 0, 0), parent=None):
    o = bpy.data.objects.new('e', None); o.location = loc
    return link(o, parent)

def anim(o, path, keys):
    """keys: (t, value[, interp, easing]) - interp/easing describe the move INTO that key."""
    byf = {}
    for k in keys: byf[F(k[0])] = k          # two keys on one frame: the later one wins
    keys = [byf[f] for f in sorted(byf)]
    for k in keys:
        setattr(o, path, k[1]); o.keyframe_insert(path, frame=F(k[0]))
    for fc in o.animation_data.action.fcurves:
        if fc.data_path != path: continue
        pts = sorted(fc.keyframe_points, key=lambda p: p.co.x)
        ks = sorted(keys, key=lambda k: k[0])
        for i in range(1, min(len(pts), len(ks))):
            pts[i - 1].interpolation = ks[i][2] if len(ks[i]) > 2 else 'LINEAR'
            pts[i - 1].easing = ks[i][3] if len(ks[i]) > 3 else 'EASE_OUT'

def life(o, t0, t1=None, s=1.0, d=0.5):
    """Pop in with overshoot at t0, snap out just before t1."""
    k = [(max(t0 - 0.04, 0), (0, 0, 0)), (t0, (0, 0, 0), 'CONSTANT'), (t0 + d, (s, s, s), 'BACK', 'EASE_OUT')]
    if t0 <= 0.001: k = [(0, (s * .6, s * .6, s * .6)), (d, (s, s, s), 'BACK', 'EASE_OUT')]
    if t1 is not None and t1 < DUR - 0.01:
        k += [(t1 - 0.2, (s, s, s), 'CONSTANT'), (t1, (0, 0, 0), 'EXPO', 'EASE_IN')]
    anim(o, 'scale', k)

def emit(col, alpha=None):
    key = (tuple(col), alpha)
    if key in mats: return mats[key]
    m = bpy.data.materials.new('c'); m.use_nodes = True; nt = m.node_tree; nt.nodes.clear()
    o = nt.nodes.new('ShaderNodeOutputMaterial'); e = nt.nodes.new('ShaderNodeEmission')
    e.inputs[0].default_value = (*col, 1); nt.links.new(e.outputs[0], o.inputs[0])
    mats[key] = m; return m

def img_mat(mode, name):
    key = (mode, name)
    if key in mats: return mats[key]
    m = bpy.data.materials.new(name); m.use_nodes = True; nt = m.node_tree; nt.nodes.clear()
    o = nt.nodes.new('ShaderNodeOutputMaterial'); e = nt.nodes.new('ShaderNodeEmission')
    t = nt.nodes.new('ShaderNodeTexImage'); t.image = bpy.data.images.load(SH % (mode, name), check_existing=True)
    t.extension = 'EXTEND'
    nt.links.new(t.outputs[0], e.inputs[0]); nt.links.new(e.outputs[0], o.inputs[0])
    mats[key] = m; return m

def soft_mat(kind, cols=None):
    """'shadow': soft dark blob; 'bg': three-colour studio gradient."""
    key = (kind, cols)
    if key in mats: return mats[key]
    m = bpy.data.materials.new(kind); m.use_nodes = True; nt = m.node_tree; nt.nodes.clear()
    N = nt.nodes.new; L = nt.links.new
    out = N('ShaderNodeOutputMaterial'); uv = N('ShaderNodeTexCoord')
    def blob(cx, cy, sc, quad=False):
        a = N('ShaderNodeVectorMath'); a.operation = 'SUBTRACT'; a.inputs[1].default_value = (cx, cy, 0)
        b = N('ShaderNodeVectorMath'); b.operation = 'SCALE'; b.inputs['Scale'].default_value = sc
        g = N('ShaderNodeTexGradient'); g.gradient_type = 'QUADRATIC_SPHERE' if quad else 'SPHERICAL'
        L(uv.outputs['UV'], a.inputs[0]); L(a.outputs[0], b.inputs[0]); L(b.outputs[0], g.inputs[0])
        return g.outputs['Fac']
    if kind == 'shadow':
        p = N('ShaderNodeMath'); p.operation = 'POWER'; p.inputs[1].default_value = 1.6
        q = N('ShaderNodeMath'); q.operation = 'MULTIPLY'; q.inputs[1].default_value = 0.62
        L(blob(.5, .5, 2.0), p.inputs[0]); L(p.outputs[0], q.inputs[0])
        tr = N('ShaderNodeBsdfTransparent'); e = N('ShaderNodeEmission'); e.inputs[0].default_value = (0, 0, 0, 1)
        mx = N('ShaderNodeMixShader'); L(q.outputs[0], mx.inputs[0]); L(tr.outputs[0], mx.inputs[1]); L(e.outputs[0], mx.inputs[2])
        L(mx.outputs[0], out.inputs[0]); m.surface_render_method = 'BLENDED'
    else:
        c1, c2, c3 = cols
        m1 = N('ShaderNodeMixRGB'); m1.inputs[1].default_value = (*c1, 1); m1.inputs[2].default_value = (*c2, 1)
        m2 = N('ShaderNodeMixRGB'); m2.inputs[2].default_value = (*c3, 1)
        L(blob(.22, .78, 1.35, True), m1.inputs[0]); L(m1.outputs[0], m2.inputs[1]); L(blob(.85, .2, 1.5, True), m2.inputs[0])
        e = N('ShaderNodeEmission'); L(m2.outputs[0], e.inputs[0]); L(e.outputs[0], out.inputs[0])
    mats[key] = m; return m

def rrect(w, h, r, uv=(0, 0, 1, 1), plane='XZ', mat=None, parent=None):
    r = max(min(r, w / 2, h / 2), 1e-4); pts = []
    for cx, cz, a0 in ((w / 2 - r, h / 2 - r, 0), (-w / 2 + r, h / 2 - r, 90), (-w / 2 + r, -h / 2 + r, 180), (w / 2 - r, -h / 2 + r, 270)):
        for k in range(9):
            a = math.radians(a0 + k * 11.25); pts.append((cx + r * math.cos(a), cz + r * math.sin(a)))
    me = bpy.data.meshes.new('rr'); bm = bmesh.new()
    f = bm.faces.new([bm.verts.new((x, 0, z) if plane == 'XZ' else (x, z, 0)) for x, z in pts]); ul = bm.loops.layers.uv.new()
    for l, (x, z) in zip(f.loops, pts):
        l[ul].uv = (uv[0] + (x / w + .5) * (uv[2] - uv[0]), uv[1] + (z / h + .5) * (uv[3] - uv[1]))
    bm.to_mesh(me); bm.free()
    o = bpy.data.objects.new('rr', me)
    if mat: me.materials.append(mat)
    return link(o, parent)

def card(mode, name, reg=None, w=3.1, parent=None, loc=(0, 0, 0)):
    """A floating slab showing (a region of) a screenshot. reg = (x, y, w, h) in 1920x1080 px."""
    x, y, rw, rh = reg or (0, 0, 1920, 1080)
    h = w * rh / rw; rad = min(w, h) * 0.055
    g = empty(loc, parent)
    rrect(w * 1.35, h * 1.35, min(w, h) * .6, mat=soft_mat('shadow'), parent=g).location = (0, .09, -h * .07)
    b = rrect(w + .05, h + .05, rad + .025, mat=emit((.035, .035, .045)), parent=g); b.location = (0, .045, 0)
    b.data.materials.append(emit((.16, .16, .2)))
    sm = b.modifiers.new('s', 'SOLIDIFY'); sm.thickness = .045; sm.offset = -1; sm.material_offset_rim = 1
    rrect(w, h, rad, uv=(x / 1920, 1 - (y + rh) / 1080, (x + rw) / 1920, 1 - y / 1080), mat=img_mat(mode, name), parent=g).location = (0, -.012, 0)
    return g

cam = bpy.data.objects.new('cam', bpy.data.cameras.new('cam')); link(cam); S.camera = cam
cam.data.lens = 35; cam.data.clip_end = 200
aim = empty()
tc = cam.constraints.new('TRACK_TO'); tc.target = aim; tc.track_axis = 'TRACK_NEGATIVE_Z'; tc.up_axis = 'UP_Y'
camk, aimk = [], []

def text(body, size, x, z, t0, t1=None, col=W, font='Bold', d=0.45, depth=5.5):
    """Camera-locked type. At depth 5.5 the frame is 3.2 wide and 5.65 tall (safe: |x|<1.35, -1.4<z<2.15)."""
    path = MONO if font == 'Mono' else FD % font
    if path not in fonts: fonts[path] = bpy.data.fonts.load(path)
    c = bpy.data.curves.new('t', 'FONT'); c.body = body; c.font = fonts[path]; c.size = size
    c.align_x = 'LEFT' if font == 'Mono' else 'CENTER'; c.align_y = 'CENTER'; c.space_line = 0.92
    o = link(bpy.data.objects.new('t', c), cam); o.location = (x, z, -depth); c.materials.append(emit(col))
    life(o, t0, t1, d=d); return o

def bg(cols, t0, t1):
    o = rrect(9, 16, .01, plane='XY', mat=soft_mat('bg', tuple(map(tuple, cols))), parent=cam); o.location = (0, 0, -14)
    anim(o, 'scale', [(0, (0, 0, 0)), (t0, (1, 1, 1), 'CONSTANT'), (t1, (0, 0, 0), 'CONSTANT')] if t0 > 0 else [(0, (1, 1, 1)), (t1, (0, 0, 0), 'CONSTANT')])

CAMS = {'L': ((-.95, -5.65, .12), (-.3, -5.45, 0), .2), 'R': ((.95, -5.65, .12), (.3, -5.45, 0), -.2),
        'P': ((0, -6.6, -.25), (0, -5.5, 0), .0), 'U': ((.25, -5.7, -.8), (0, -5.5, .05), -.1), 'S': ((0, -5.5, 0), (0, -5.5, 0), 0)}
nshot = [0]

def cam_to(O, t0, t1, preset):
    a, b, yaw = CAMS[preset]
    camk.extend([(t0, tuple(O + Vector(a)), 'CONSTANT'), (t1 - 1 / FPS, tuple(O + Vector(b)), 'SINE', 'EASE_OUT')])
    aimk.extend([(t0, tuple(O), 'CONSTANT'), (t1 - 1 / FPS, tuple(O), 'CONSTANT')])
    return yaw

def shot(t0, t1, cols, back=None, panels=(), cam_='L', title=None, sub=None, acc=W, tsize=.3):
    nshot[0] += 1; O = Vector((nshot[0] * 60, 0, 0)); bg(cols, t0, t1)
    g = empty(O); g.rotation_euler = (0.04, 0, cam_to(O, t0, t1, cam_))
    if back:
        life(card(back[0], back[1], None, 3.1, g, (0, .7, .5)), t0, d=.4)
    for k, (mode, name, reg, w, pos) in enumerate(panels):
        life(card(mode, name, reg, w, g, pos), t0 + .1 + k * .14, d=.55)
    if title: text(title, tsize, 0, 2.02 if '\n' not in title else 1.9, t0 + .04, t1)
    if sub: text(sub, .155, 0, 1.7 if title and '\n' not in title else 1.45, t0 + .22, t1, col=acc, font='SemiBold')
    return O

def hook(t1, body, cols, sub=None, acc=W, size=.4):
    nshot[0] += 1; O = Vector((nshot[0] * 60, 0, 0)); bg(cols, 0, t1); cam_to(O, 0, t1, 'S')
    lines = body.split('\n')
    for i, ln in enumerate(lines):
        text(ln, size, 0, .55 + (len(lines) - 1) * size * .5 - i * size * 1.05, 0.05 + i * .42, t1, d=.4, col=acc if (i == len(lines) - 1 and i > 0) else W)
    if sub: text(sub, .15, 0, .55 - len(lines) * size * .62 - .1, len(lines) * .42 + .1, t1, col=acc, font='SemiBold')

def outro(t0, line='Seven looks. One desktop.', cols=((.03, .02, .02), (.5, .2, .1), (.3, .1, .2)), acc=(1, .71, .58)):
    nshot[0] += 1; O = Vector((nshot[0] * 60, 0, 0)); bg(cols, t0, DUR + 1); cam_to(O, t0, DUR + .1, 'S')
    text('Creed', .86, 0, .75, t0 + .05, d=.5, font='ExtraBold')
    text(line, .2, 0, .02, t0 + .3, font='SemiBold')
    text('Free & open source  ·  Arch + Hyprland', .125, 0, -.38, t0 + .5, col=acc, font='Medium')
    text('github.com/UmxRK/creedfiles', .12, 0, -.72, t0 + .7, col=(.62, .62, .68), font='Medium')

# ── palettes and screen regions (1920×1080 logical px) ──
C = {'creed': (((.07, .035, .02), (.62, .26, .13), (.4, .13, .22)), (1, .71, .58)),
     'win11': (((.01, .025, .09), (0, .22, .8), (.08, .45, .9)), (.3, .76, 1)),
     'win10': (((0, .02, .07), (0, .28, .62), (0, .14, .4)), (.35, .68, .95)),
     'mac-tahoe': (((.02, .05, .15), (.05, .33, .9), (.3, .68, .95)), (.55, .8, 1)),
     'mac-sequoia': (((.06, .025, .15), (.48, .2, .75), (.92, .38, .4)), (1, .7, .66)),
     'mac-bigsur': (((.09, .02, .09), (.85, .18, .33), (.18, .42, .9)), (1, .62, .5)),
     'mac-aqua': (((.02, .045, .13), (.1, .3, .75), (.45, .66, .95)), (.6, .8, 1))}
RS, LS = (1484, 44, 436, 1036), (0, 44, 436, 1036)
H = {
 'creed': ('creed', '00-desktop', [('creed', '10-control', RS, .95, (.62, -1.0, -1.0)), ('creed', '10-dashboard', LS, .95, (-.64, -.7, -1.2))]),
 'creed.launch': ('creed', '00-desktop', [('creed', '21-launcher-search', (610, 164, 700, 526), 2.3, (0, -1, -1.05)), ('creed', '30-osd-volume', (840, 44, 240, 72), 1.3, (0, -1.4, -2.2))]),
 'creed.calc': ('creed', None, [('creed', '22-launcher-calc', (616, 168, 380, 124), 2.5, (0, -1, 0))]),
 'creed.osd': ('creed', None, [('creed', '30-osd-volume', (840, 44, 240, 72), 2.4, (0, -1, 0))]),
 'win11': ('win11', '00-desktop', [('win11', '10-launcher', (630, 286, 660, 744), 1.9, (0, -1, -.95))]),
 'win11.side': ('win11', '10-dashboard', [('win11', '10-notif-center', (1540, 424, 376, 604), 1.25, (.7, -1, -1.25)), ('win11', '10-control', (1540, 752, 376, 276), 1.25, (-.72, -.8, -1.5))]),
 'win11.cal': ('win11', None, [('win11', '10-notif-center', (1540, 640, 376, 388), 2.4, (0, -1, 0))]),
 'win10': ('win10', '00-desktop', [('win10', '10-launcher', (0, 400, 700, 640), 2.2, (0, -1, -1.0))]),
 'win10.tiles': ('win10', None, [('win10', '10-launcher', (320, 420, 340, 400), 2.3, (0, -1, 0))]),
 'mac-tahoe': ('mac-tahoe', '00-desktop', [('mac-tahoe', '10-control', (1584, 24, 336, 326), 1.5, (.55, -1, -.95)), ('mac-tahoe', '00-desktop', (20, 36, 354, 522), 1.1, (-.75, -.7, -1.5))]),
 'mac-tahoe.sp': ('mac-tahoe', '00-desktop', [('mac-tahoe', '21-launcher-search', (632, 208, 656, 280), 2.5, (0, -1, -.95)), ('mac-tahoe', '00-desktop', (800, 992, 320, 84), 1.9, (0, -1.3, -1.75))]),
 'mac-tahoe.wid': ('mac-tahoe', None, [('mac-tahoe', '00-desktop', (20, 36, 354, 522), 2.0, (0, -1, 0))]),
 'mac-tahoe.menu': ('mac-tahoe', None, [('mac-tahoe', '40-menu-edit', (140, 0, 240, 262), 2.2, (0, -1, 0))]),
 'mac-tahoe.nc': ('mac-tahoe', '00-desktop', [('mac-tahoe', '10-notif-center', (1560, 24, 356, 540), 1.5, (0, -1, -1.2))]),
 'mac-sequoia': ('mac-sequoia', '00-desktop', [('mac-sequoia', '10-control', (1584, 24, 336, 326), 1.5, (.5, -1, -1.0)), ('mac-sequoia', '10-notif-center', (1560, 24, 356, 540), 1.1, (-.75, -.7, -1.45))]),
 'mac-bigsur': ('mac-bigsur', '00-desktop', [('mac-bigsur', '10-control', (1584, 24, 336, 326), 1.5, (-.5, -1, -1.0)), ('mac-bigsur', '30-osd-volume', (852, 722, 216, 216), .95, (.78, -1.2, -1.6))]),
 'mac-aqua': ('mac-aqua', '00-desktop', [('mac-aqua', '21-launcher-search', (1570, 0, 350, 196), 2.2, (0, -1, -.85)), ('mac-aqua', '10-dashboard', (610, 360, 790, 300), 2.4, (0, -.8, -1.95))]),
 'mac-aqua.hud': ('mac-aqua', None, [('mac-aqua', '30-osd-volume', (852, 722, 216, 216), 1.9, (0, -1, 0))]),
 'mac-aqua.extra': ('mac-aqua', '00-desktop', [('mac-aqua', '10-control', (1656, 0, 264, 160), 2.3, (0, -1, -1.0)), ('mac-aqua', '00-desktop', (800, 990, 320, 90), 2.0, (0, -1.3, -2.0))]),
}

def seq(t, items, end):
    """items: (key, seconds, title, sub, cam). Returns the time after the last shot."""
    for key, d, title, sub, c in items:
        mode, back, panels = H[key]; cols, acc = C[mode]
        shot(t, min(t + d, end), cols, (mode, back) if back else None, panels, c, title, sub, acc); t += d
    return t

def bar(x, h, col, t0, t1, w=.62):
    o = rrect(w, h, .06, plane='XY', mat=emit(col), parent=cam); o.location = (x, -1.3 + h / 2, -5.5)
    for v in o.data.vertices: v.co.y += h / 2
    o.location = (x, -1.3, -5.5)
    anim(o, 'scale', [(max(t0 - .04, 0), (1, 0, 1)), (t0, (1, 0, 1), 'CONSTANT'), (t0 + .9, (1, 1, 1), 'EXPO', 'EASE_OUT'), (t1 - .2, (1, 1, 1), 'CONSTANT'), (t1, (0, 0, 0), 'EXPO', 'EASE_IN')])

def uicard(x, z, w, h, col, t0, t1, lines, r=.09):
    g = empty((x, z, -5.5), cam); rrect(w, h, r, plane='XY', mat=emit(col), parent=g)
    for body, size, c, dx, dz, font in lines:
        path = MONO if font == 'Mono' else FD % font
        if path not in fonts: fonts[path] = bpy.data.fonts.load(path)
        cu = bpy.data.curves.new('t', 'FONT'); cu.body = body; cu.font = fonts[path]; cu.size = size; cu.align_x = 'LEFT'; cu.align_y = 'CENTER'
        o = link(bpy.data.objects.new('t', cu), g); o.location = (-w / 2 + dx, dz, .01); cu.materials.append(emit(c))
    life(g, t0, t1); return g

V = NAME
if V == 'seven':
    hook(1.7, 'One desktop.\nSeven looks.', C['creed'][0], acc=C['creed'][1])
    t = seq(1.7, [('creed', 1.55, 'Creed', 'Material You', 'L'), ('win11', 1.55, 'Windows 11', 'Start, Snap, Widgets', 'R'),
                  ('win10', 1.55, 'Windows 10', 'Tiles and Action Center', 'L'), ('mac-tahoe', 1.55, 'macOS Tahoe', 'Liquid Glass', 'R'),
                  ('mac-sequoia', 1.55, 'macOS Sequoia', 'Control Center', 'L'), ('mac-bigsur', 1.55, 'macOS Big Sur', 'Bright and bold', 'R'),
                  ('mac-aqua', 1.55, 'Classic Aqua', 'Straight from 2005', 'P')], 15)
    outro(t)
elif V == 'ram':
    cols, acc = C['creed']; nshot[0] += 1; O = Vector((60, 0, 0)); bg(cols, 0, 7.2); cam_to(O, 0, 7.2, 'S')
    n = text('0', 1.5, -.25, .7, .05, 3.0, font='ExtraBold'); dyn.append((n, lambda t: str(min(28, int(28 * min(t / 1.1, 1) ** .6)))))
    text('MB', .42, .9, .42, .5, 3.0, col=acc); text('of RAM. Everything on screen.', .16, 0, -.45, 1.2, 3.0, font='SemiBold')
    text('The budget was 60.', .26, 0, 2.0, 3.1, 7.2); bar(-.55, 2.6, (.3, .3, .34), 3.3, 7.2); bar(.55, 1.21, acc, 3.8, 7.2)
    text('60 MB', .2, -.55, 1.5, 3.9, 7.2, col=(.75, .75, .8)); text('28 MB', .2, .55, .12, 4.5, 7.2, col=acc)
    text('budget', .12, -.55, -1.5, 3.5, 7.2, col=(.6, .6, .65), font='Medium'); text('Creed', .12, .55, -1.5, 4.0, 7.2, col=(.6, .6, .65), font='Medium')
    t = seq(7.2, [('creed', 3.2, 'Opened 200 panels.', 'Still 28 MB', 'L')], 15)
    nshot[0] += 1; bg(C['win11'][0], t, 12.6); cam_to(Vector((nshot[0] * 60, 0, 0)), t, 12.6, 'S')
    text('0%', 1.3, 0, .6, t + .05, 12.6, font='ExtraBold'); text('CPU when idle', .2, 0, -.4, t + .35, 12.6, col=C['win11'][1], font='SemiBold')
    text('All seven looks: 27–33 MB at 1080p', .11, 0, -.85, t + .6, 12.6, col=(.6, .6, .68), font='Medium')
    outro(12.6, 'Light enough to forget.')
elif V == 'glass':
    hook(1.8, 'Liquid Glass.\nOn Linux.', C['mac-tahoe'][0], acc=C['mac-tahoe'][1])
    t = seq(1.8, [('mac-tahoe', 3.1, 'Control Center', 'and desktop widgets', 'L'), ('mac-tahoe.sp', 3.0, 'Spotlight', 'one capsule, one panel', 'R'),
                  ('mac-tahoe.nc', 2.8, 'Notifications', 'with a calendar underneath', 'U')], 15)
    nshot[0] += 1; bg(C['mac-tahoe'][0], t, 12.7); cam_to(Vector((nshot[0] * 60, 0, 0)), t, 12.7, 'S')
    text('Every knob\nis yours.', .4, 0, .7, t + .05, 12.7); text('refraction  ·  frost  ·  tint  ·  shine', .14, 0, -.1, t + .4, 12.7, col=C['mac-tahoe'][1], font='SemiBold')
    outro(12.7, 'macOS Tahoe mode.', C['mac-tahoe'][0], C['mac-tahoe'][1])
elif V == 'win11':
    hook(1.8, 'This is\nnot Windows.', C['win11'][0], acc=C['win11'][1])
    t = seq(1.8, [('win11', 3.2, "It's Arch Linux.", 'Start menu included', 'L'), ('win11.side', 3.2, 'Quick Settings', 'calendar and notifications', 'R'),
                  ('win10', 2.9, 'Miss Windows 10?', "That's a mode too", 'P')], 15)
    nshot[0] += 1; bg(C['win11'][0], t, 12.7); cam_to(Vector((nshot[0] * 60, 0, 0)), t, 12.7, 'S')
    text('No ads.\nNo telemetry.', .38, 0, .6, t + .05, 12.7); text('Just your desktop', .15, 0, -.15, t + .4, 12.7, col=C['win11'][1], font='SemiBold')
    outro(12.7, 'Windows 11 mode.', C['win11'][0], C['win11'][1])
elif V == 'material':
    hook(1.9, 'Your wallpaper\npicks the colours.', C['creed'][0], acc=C['creed'][1], size=.34)
    t = seq(1.9, [('creed', 3.4, 'Material You', 'for Hyprland', 'L'), ('creed.launch', 3.0, 'Search. Launch.', 'calculate and run commands', 'R'),
                  ('creed', 3.0, 'Sidebars', 'toggles, calendar, to-do, media', 'U')], 15)
    nshot[0] += 1; bg(C['creed'][0], t, 12.9); cam_to(Vector((nshot[0] * 60, 0, 0)), t, 12.9, 'S')
    for i, c in enumerate(((1, .71, .58), (.45, .21, .07), (.36, .25, .2), (.85, .77, .55), (.1, .07, .05))):
        ch = rrect(.44, .44, .22, plane='XY', mat=emit(c), parent=cam); ch.location = (-1.1 + i * .55, .5, -5.5)
        life(ch, t + .05 + i * .09, 12.9)
    text('One palette, everywhere.', .2, 0, -.1, t + .5, 12.9, font='SemiBold')
    outro(12.9, 'Creed mode.')
elif V == 'aqua':
    hook(1.7, '2005 called.', C['mac-aqua'][0], 'It wants its desktop back', C['mac-aqua'][1])
    t = seq(1.7, [('mac-aqua', 3.3, 'Spotlight', 'the Tiger way', 'L'), ('mac-aqua.hud', 2.5, 'That volume HUD', None, 'P'),
                  ('mac-aqua.extra', 3.0, 'Menu extras', 'and the shelf dock', 'R'), ('mac-bigsur', 2.3, 'Too retro?', 'Big Sur is one click away', 'L')], 15)
    outro(t, 'Classic Aqua mode.', C['mac-aqua'][0], C['mac-aqua'][1])
elif V == 'command':
    cols, acc = C['mac-tahoe']; nshot[0] += 1; O = Vector((60, 0, 0)); bg(((.02, .02, .03), (.12, .12, .2), (.2, .1, .25)), 0, 12.6); gg = empty(O); gg.rotation_euler = (.05, 0, cam_to(O, 0, 12.6, 'L'))
    text('One command.', .4, 0, 2.0, .05, 12.6)
    life(card('creed', '00-desktop', None, 3.0, gg, (0, 0, .7)), .2, 3.2)
    cmds = [(1.2, 'creed mode set win11', 'win11', 3.2), (5.0, 'creed mode set mac-tahoe', 'mac-tahoe', 6.8), (8.6, 'creed mode set mac-aqua', 'mac-aqua', 10.3)]
    for i, (_, _, mode, at) in enumerate(cmds):
        life(card(mode, '00-desktop', None, 3.0, gg, (0, 0, .7)), at, cmds[i + 1][3] if i < 2 else 12.6)
    uicard(0, -1.0, 2.9, .62, (.025, .025, .035), .4, 12.6, [], r=.1)
    tt = text('$ ', .17, -1.3, -1.0, .5, 12.6, col=(.75, 1, .8), font='Mono')
    def typed(t):
        s = '$ '
        for st, c, _, at in cmds:
            if t >= st: s = '$ ' + c[:int((t - st) * 16)] + ('' if t >= at + .9 else '')
        return s + ('▌' if int(t * 3) % 2 == 0 else '')
    dyn.append((tt, typed))
    text('Bar, dock, menus, shortcuts, apps, lock screen', .105, 0, -1.6, 1.0, 12.6, col=(.65, .65, .72), font='Medium')
    outro(12.6, 'Switch any time.')
elif V == 'tweaks':
    cols = ((.02, .04, .05), (.05, .35, .3), (.1, .2, .4)); acc = (.45, .95, .75)
    nshot[0] += 1; bg(cols, 0, 12.6); cam_to(Vector((60, 0, 0)), 0, 12.6, 'S')
    text('It asks before', .3, 0, 2.05, .05, 12.6); text('it touches anything.', .3, 0, 1.72, .3, 12.6, col=acc)
    rows = [('Faster TCP: BBR', 'Network'), ('Compressed RAM swap (zram)', 'System'), ('Raw mouse input', 'Gaming'), ('Tune GameMode', 'Gaming')]
    for i, (a, b) in enumerate(rows):
        uicard(0, 1.05 - i * .5, 2.8, .42, (.09, .11, .13), .8 + i * .22, 5.6, [(a, .13, W, .16, .05, 'SemiBold'), (b, .085, (.6, .7, .7), .16, -.1, 'Medium'), ('Undo', .1, acc, 2.25, 0, 'SemiBold')])
    uicard(0, .2, 2.8, 2.1, (.09, .11, .13), 5.7, 9.6, [('Clear app caches', .19, W, .2, .78, 'Bold'),
        ('Deletes cache folders that apps rebuild', .12, (.82, .86, .86), .2, .42, 'Medium'), ('on their own. Your settings, logins', .12, (.82, .86, .86), .2, .24, 'Medium'),
        ('and files are not touched.', .12, (.82, .86, .86), .2, .06, 'Medium'), ('Reversible: no', .12, (1, .7, .5), .2, -.3, 'SemiBold'),
        ('Run again with --confirm', .115, acc, .2, -.62, 'Mono')])
    text('Explained first. Every time.', .16, 0, -1.25, 6.2, 9.6, font='SemiBold')
    text('26', 1.3, 0, .6, 9.7, 12.6, font='ExtraBold'); text('tweaks for games, Wi-Fi, battery', .16, 0, -.35, 10.0, 12.6, font='SemiBold')
    text('19 undo with one click', .14, 0, -.7, 10.3, 12.6, col=acc, font='SemiBold')
    outro(12.6, 'Tuned, never broken.', cols, acc)
elif V == 'era':
    hook(1.5, 'Pick your era.', C['mac-aqua'][0], acc=C['mac-aqua'][1])
    t = seq(1.5, [('mac-aqua', 1.55, '2005', 'Classic Aqua', 'L'), ('win10', 1.55, '2015', 'Windows 10', 'R'), ('mac-bigsur', 1.55, '2020', 'macOS Big Sur', 'L'),
                  ('win11', 1.55, '2021', 'Windows 11', 'R'), ('mac-sequoia', 1.55, '2024', 'macOS Sequoia', 'L'), ('mac-tahoe', 1.55, '2025', 'macOS Tahoe', 'R'),
                  ('creed', 1.7, 'Now', 'Creed', 'P')], 15)
    outro(t, 'Every era. One install.')
elif V == 'details':
    hook(1.5, "It's the\ndetails.", C['mac-sequoia'][0], acc=C['mac-sequoia'][1])
    t = seq(1.5, [('creed.osd', 1.2, 'Volume', None, 'P'), ('win11.cal', 1.25, 'Calendar', None, 'L'), ('mac-tahoe.wid', 1.25, 'Widgets', None, 'R'),
                  ('mac-aqua.hud', 1.2, 'HUD', None, 'P'), ('win10.tiles', 1.25, 'Tiles', None, 'L'), ('mac-tahoe.menu', 1.25, 'Menus', None, 'R'),
                  ('creed.calc', 1.25, 'Calculator', None, 'P'), ('win11', 1.3, 'Start', None, 'L'), ('mac-sequoia', 1.3, 'Control Center', None, 'R')], 15)
    outro(t, 'Sweat the small stuff.', C['mac-sequoia'][0], C['mac-sequoia'][1])
elif V == 'game':
    cols = ((.03, .015, .06), (.45, .08, .6), (.08, .45, .9)); acc = (.55, 1, .7)
    nshot[0] += 1; bg(cols, 0, 7.6); cam_to(Vector((60, 0, 0)), 0, 7.6, 'S')
    text('Game Mode.', .42, 0, 2.0, .05, 7.6); text('Everything steps aside for the game', .15, 0, 1.6, .35, 7.6, col=acc, font='SemiBold')
    rows = [('Blur and shadows', 'off'), ('Animations', 'off'), ('Bar and dock', 'unloaded'), ('CPU governor', 'performance'), ('Tearing', 'allowed'), ('Starts', 'by itself')]
    for i, (a, b) in enumerate(rows):
        uicard(0, 1.0 - i * .47, 2.8, .4, (.07, .06, .1), .9 + i * .5, 7.6, [(a, .14, W, .18, 0, 'SemiBold'), (b, .13, acc, 1.75, 0, 'Bold')])
    H['creed.game'] = ('creed', None, [('creed', '30-osd-game', (840, 44, 240, 72), 2.5, (0, -1, 0))])
    t = seq(7.6, [('creed.game', 2.2, 'One key.', 'or automatic when a game opens', 'P')], 15)
    nshot[0] += 1; bg(cols, t, 12.6); cam_to(Vector((nshot[0] * 60, 0, 0)), t, 12.6, 'S')
    text('More frames.\nLess lag.', .42, 0, .7, t + .05, 12.6); text('Measure it yourself: MangoHud is built in', .13, 0, -.2, t + .45, 12.6, col=acc, font='SemiBold')
    outro(12.6, 'Built for play.', cols, acc)
elif V == 'stats':
    segs = [('creed', lambda u: f'{int(28 * u)} MB', 'of RAM, with everything on screen'), ('win11', lambda u: f'{int(100 - 100 * u)}%', 'CPU when idle'),
            ('mac-tahoe', lambda u: f'{0.9 - 0.75 * u:.2f} s', 'to open any panel'), ('mac-sequoia', lambda u: f'{1 + int(6 * u)}', 'looks in one install'),
            ('win10', lambda u: f'{int(26 * u)}', 'tweaks, each explained first'), ('mac-bigsur', lambda u: f'{9 - int(8 * u)}', 'command to switch looks')]
    for i, (pal, fn, cap) in enumerate(segs):
        t0, t1 = i * 2.05, (i + 1) * 2.05; nshot[0] += 1; bg(C[pal][0], t0, t1); cam_to(Vector((nshot[0] * 60, 0, 0)), t0, t1, 'S')
        n = text('0', .72 if i in (0, 2) else 1.15, 0, .7, t0 + .03, t1, font='ExtraBold', d=.35)
        dyn.append((n, (lambda fn, t0, t1: lambda t: fn(min(max((t - t0) / .8, 0), 1) ** .5) if t0 - .1 <= t < t1 + .1 else None)(fn, t0, t1)))
        text(cap, .17, 0, -.05, t0 + .5, t1, col=C[pal][1], font='SemiBold')
        text(f'{i + 1} / 6', .1, 0, 2.05, t0 + .05, t1, col=(.7, .7, .76), font='Medium')
    outro(12.3, 'The numbers are real.')
else:
    raise SystemExit('unknown video ' + V)

anim(cam, 'location', sorted(camk, key=lambda k: k[0])); anim(aim, 'location', sorted(aimk, key=lambda k: k[0]))

def tick(scene, *_):
    t = (scene.frame_current - 1) / FPS
    for o, fn in dyn:
        b = fn(t)
        if b is not None: o.data.body = b
bpy.app.handlers.frame_change_pre.append(tick)

d = f'{OUT}/{V}'; os.makedirs(d, exist_ok=True)
S.render.image_settings.file_format = 'JPEG'; S.render.image_settings.quality = 96
if KIND == 'still':
    for t in A[2].split(','):
        S.frame_set(F(float(t))); S.render.filepath = f'{d}/still_{float(t):05.2f}.jpg'; bpy.ops.render.render(write_still=True)
else:
    S.render.filepath = f'{d}/f_'; S.render.use_overwrite = False
    bpy.ops.render.render(animation=True)

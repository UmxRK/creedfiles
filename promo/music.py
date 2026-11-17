"""Original music bed for the promo compilations (synthesised, no samples, no licences).
120 BPM, A minor (Am–F–C–G), four-on-the-floor kick, clap on 2 and 4, off-beat hats, sub bass
that ducks under the kick, plucked chords, a riser into every cut and an impact on it.
Usage: python3 promo/music.py OUT.wav DURATION CUT[,CUT...]   (times in seconds, on the beat grid)"""
import sys
import wave

import numpy as np

SR = 48000
BPM = 120
BEAT = 60 / BPM
rng = np.random.default_rng(7)


def env(n, a, d):
    """Attack/exponential-decay envelope of n samples."""
    t = np.arange(n) / SR
    e = np.exp(-t / d)
    na = max(1, int(a * SR))
    e[:na] *= np.linspace(0, 1, na)
    return e


def add(buf, sig, at, gain=1.0):
    i = int(round(at * SR))
    if i < 0:                       # starts before the track: keep only the audible tail
        sig, i = sig[-i:], 0
    if i >= len(buf) or len(sig) == 0:
        return
    j = min(len(buf), i + len(sig))
    buf[i:j] += sig[: j - i] * gain


def kick():
    n = int(.45 * SR); t = np.arange(n) / SR
    f = 42 + 120 * np.exp(-t / .045)
    ph = 2 * np.pi * np.cumsum(f) / SR
    return np.sin(ph) * env(n, .001, .16) + .25 * np.sin(ph * 2) * env(n, .001, .03)


def clap():
    n = int(.3 * SR); s = rng.standard_normal(n)
    e = np.zeros(n)
    for k, o in enumerate((0, .011, .022)):
        e[int(o * SR):] += env(n - int(o * SR), .0005, .012 if k < 2 else .09)
    s = np.convolve(s, np.ones(6) / 6, 'same') - np.convolve(s, np.ones(40) / 40, 'same')
    return s * e * .9


def hat(open_=False):
    n = int((.18 if open_ else .05) * SR); s = rng.standard_normal(n)
    s = s - np.convolve(s, np.ones(8) / 8, 'same')
    return s * env(n, .0005, .06 if open_ else .012) * .35


def saw(f, n):
    t = np.arange(n) / SR
    return 2 * ((t * f) % 1) - 1


def lowpass(x, a):
    y = np.empty_like(x); acc = 0.0
    for i, v in enumerate(x):  # one-pole, fine for short notes
        acc += a * (v - acc); y[i] = acc
    return y


def pluck(freqs, length=BEAT * 1.6):
    n = int(length * SR); s = np.zeros(n)
    for f in freqs:
        s += saw(f, n) + saw(f * 1.006, n) * .7
    s = lowpass(s, .09) * env(n, .004, .32)
    return s / len(freqs) * .5


def sub(f, length):
    n = int(length * SR); t = np.arange(n) / SR
    return np.sin(2 * np.pi * f * t) * env(n, .01, length * 2) * .55


def riser(length):
    n = int(length * SR); t = np.arange(n) / SR
    s = rng.standard_normal(n)
    cut = .02 + .5 * (t / length) ** 2
    out = np.empty(n); acc = 0.0
    for i in range(n):
        acc += cut[i] * (s[i] - acc); out[i] = acc
    return out * (t / length) ** 2 * .5


def impact():
    n = int(1.2 * SR); t = np.arange(n) / SR
    boom = np.sin(2 * np.pi * (38 + 60 * np.exp(-t / .08)) * t) * env(n, .001, .35)
    air = rng.standard_normal(n) * env(n, .001, .25)
    air = air - np.convolve(air, np.ones(30) / 30, 'same')
    return boom * .9 + air * .25


def main():
    out, dur, cuts = sys.argv[1], float(sys.argv[2]), [float(c) for c in sys.argv[3].split(',') if c]
    n = int(dur * SR); L = np.zeros(n); R = np.zeros(n); duck = np.ones(n)
    roots = [110.0, 87.31, 130.81, 98.0]                       # A2 F2 C3 G2
    chords = [[220, 261.63, 329.63], [174.61, 220, 261.63], [261.63, 329.63, 392], [196, 246.94, 293.66]]
    K, C, H, Ho = kick(), clap(), hat(), hat(True)
    beats = int(dur / BEAT)
    intro = 3  # beats of teaser: drums only, then everything comes in on the first cut
    for b in range(beats):
        t = b * BEAT; bar = (b // 4) % 4
        add(L, K, t, .95); add(R, K, t, .95)
        k = int(t * SR); m = min(n, k + int(.22 * SR))
        duck[k:m] = np.minimum(duck[k:m], .25 + .75 * np.linspace(0, 1, m - k))
        if b % 2 == 1:
            add(L, C, t, .55); add(R, C, t + .004, .55)
        add(L, Ho if b % 4 == 3 else H, t + BEAT / 2, .8); add(R, Ho if b % 4 == 3 else H, t + BEAT / 2 + .003, .7)
        if b >= intro:
            if b % 4 == 0:
                s = sub(roots[bar], BEAT * 4); add(L, s, t); add(R, s, t)
            if b % 2 == 0:
                p = pluck(chords[bar]); add(L, p, t + .01, .9); add(R, p, t, .75)
    for c in cuts:
        r = riser(BEAT * 4); add(L, r, c - BEAT * 4); add(R, r, c - BEAT * 4)
        im = impact(); add(L, im, c); add(R, im, c)
    L *= duck ** .5; R *= duck ** .5
    # Fade the tail into the loop point so the video loops cleanly.
    f = int(.5 * SR); L[-f:] *= np.linspace(1, 0, f); R[-f:] *= np.linspace(1, 0, f)
    mix = np.stack([L, R], 1)
    mix *= .9 / np.max(np.abs(mix))
    mix = np.tanh(mix * 1.15) / np.tanh(1.15)                  # light glue; loudness is set later (EBU R128)
    mix *= .5 / np.max(np.abs(mix))
    with wave.open(out, 'wb') as w:
        w.setnchannels(2); w.setsampwidth(2); w.setframerate(SR)
        w.writeframes((mix * 32767).astype('<i2').tobytes())


if __name__ == '__main__':
    main()

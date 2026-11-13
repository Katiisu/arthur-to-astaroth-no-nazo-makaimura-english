"""Generates assets/castle.png: original pixel art of a castle on a crag in a thunderstorm.
220x130 pixels, shown at 4x in the patcher. Rain and lightning are animated in code.
Run: python3 tools/make_castle.py  (needs Pillow + numpy)"""
import numpy as np
from PIL import Image

W, H = 220, 130
rng = np.random.default_rng(1996)

C = {
    'sky0': (11, 7, 22), 'sky1': (22, 13, 42), 'sky2': (38, 22, 66), 'sky3': (58, 34, 88),
    'cl0': (24, 18, 44), 'cl1': (36, 28, 62), 'cl2': (54, 44, 88), 'cl3': (80, 68, 120),
    'mtn': (26, 18, 46), 'mtn2': (20, 14, 36),
    'rock': (17, 12, 30), 'rockhi': (34, 26, 56), 'rockdk': (10, 7, 18),
    'wall': (21, 16, 36), 'wallhi': (40, 31, 64), 'walldk': (13, 10, 24),
    'roof': (30, 16, 44), 'roofhi': (52, 28, 72),
    'win': (255, 204, 77), 'win2': (255, 148, 46), 'glow': (90, 52, 40),
    'ground': (9, 6, 16), 'grass': (22, 30, 34), 'flag': (150, 34, 52),
    'stone': (36, 30, 52), 'stonehi': (56, 48, 78),
}
img = np.zeros((H, W, 3), np.uint8)
BAYER = np.array([[0, 8, 2, 10], [12, 4, 14, 6], [3, 11, 1, 9], [15, 7, 13, 5]]) / 16.0


def px(x, y, c):
    if 0 <= x < W and 0 <= y < H:
        img[y, x] = C[c] if isinstance(c, str) else c


def rect(x0, y0, x1, y1, c):
    for y in range(max(0, y0), min(H, y1)):
        for x in range(max(0, x0), min(W, x1)):
            px(x, y, c)


# --- sky: dithered gradient
stops = ['sky0', 'sky1', 'sky2', 'sky3']
for y in range(H):
    t = min(y / 100.0, 0.999) * (len(stops) - 1)
    i, f = int(t), t - int(t)
    for x in range(W):
        img[y, x] = C[stops[i + 1]] if f > BAYER[y % 4, x % 4] else C[stops[i]]


# --- storm clouds: value noise, thicker near the top
def noise(scale, seed):
    r = np.random.default_rng(seed)
    gw, gh = W // scale + 2, H // scale + 2
    g = r.random((gh, gw))
    ys, xs = np.mgrid[0:H, 0:W] / scale
    x0, y0 = xs.astype(int), ys.astype(int)
    fx, fy = xs - x0, ys - y0
    fx, fy = fx * fx * (3 - 2 * fx), fy * fy * (3 - 2 * fy)
    a = g[y0, x0] * (1 - fx) + g[y0, x0 + 1] * fx
    b = g[y0 + 1, x0] * (1 - fx) + g[y0 + 1, x0 + 1] * fx
    return a * (1 - fy) + b * fy


n = 0.65 * noise(26, 3) + 0.27 * noise(11, 4) + 0.08 * noise(5, 5)
ys = np.mgrid[0:H, 0:W][0]
dens = n + 0.42 - ys / 115.0
for y in range(H):
    for x in range(W):
        d = dens[y, x] + (BAYER[y % 4, x % 4] - 0.5) * 0.02
        if d > 0.62:
            img[y, x] = C['cl0']
        elif d > 0.54:
            img[y, x] = C['cl1']
        elif d > 0.49:
            img[y, x] = C['cl2']

# --- moon glowing through the storm, backlighting the castle
MX, MY = 60, 20
for y in range(H):
    for x in range(W):
        r = ((x - MX) ** 2 + (y - MY) ** 2) ** 0.5
        if r < 9:
            img[y, x] = (214, 200, 236) if (x - MX) + (y - MY) < 6 else (178, 160, 210)
        elif r < 34:
            f = (34 - r) / 25.0
            if f * 0.9 > BAYER[y % 4, x % 4]:
                img[y, x] = np.minimum(255, img[y, x].astype(int) + (int(40 * f) + 14, int(30 * f) + 10, int(48 * f) + 16))
# a cloud band drifting across the moon
for y in range(H):
    for x in range(W):
        band = 22 + 3 * np.sin(x / 7.0)
        if abs(y - band) < 2.2 + n[y, x] * 2 and 30 < x < 100:
            img[y, x] = C['cl1'] if y < band else C['cl0']

# --- distant mountains
h = 92.0
for x in range(W):
    h += rng.normal(0, 1.1)
    h = min(max(h, 80), 100)
    top = int(h - 6 * np.sin(x / 17.0))
    for y in range(top, H):
        px(x, y, 'mtn' if y < top + 2 else 'mtn2')

# --- crag under the castle (left side)
for x in range(0, 112):
    edge = 82 + int(0.0045 * (x - 48) ** 2) + int(rng.integers(0, 2))
    if x > 96:
        edge = 82 + int(0.0045 * (x - 48) ** 2) + (x - 96) * 2
    for y in range(edge, H):
        c = 'rock'
        if y == edge or (x < 50 and rng.random() < 0.08):
            c = 'rockhi'
        elif rng.random() < 0.06:
            c = 'rockdk'
        px(x, y, c)

# --- castle
def stone_fill(x0, y0, x1, y1):
    for y in range(y0, y1):
        for x in range(x0, x1):
            c = 'wall'
            if (y - y0) % 4 == 3 and rng.random() < 0.7:
                c = 'walldk'
            elif x == x0:
                c = 'wallhi'   # left rim lit by the storm
            elif rng.random() < 0.05:
                c = 'wallhi'
            px(x, y, c)


def battlements(x0, x1, y):
    for x in range(x0, x1):
        if (x - x0) % 3 != 2:
            px(x, y - 1, 'wall')
            px(x, y - 2, 'wall' if x != x0 else 'wallhi')


def roof(cx, y_base, half, apex):
    for y in range(apex, y_base):
        w = int((y - apex) / (y_base - apex) * half)
        for x in range(cx - w, cx + w + 1):
            px(x, y, 'roofhi' if x <= cx - w + 1 else 'roof')
    px(cx, apex - 1, 'roof')


def window(x, y, tall=3):
    for k in range(tall):
        px(x, y + k, 'win' if k else 'win2')
    px(x - 1, y + 1, 'glow')
    px(x + 1, y + 1, 'glow')


# curtain wall
stone_fill(14, 62, 92, 84)
battlements(14, 92, 62)
# keep
stone_fill(38, 38, 66, 84)
battlements(38, 66, 38)
# left tall tower
stone_fill(20, 26, 32, 84)
roof(26, 26, 8, 8)
# right tower
stone_fill(70, 34, 80, 84)
roof(75, 34, 7, 18)
# keep turret
stone_fill(48, 24, 56, 38)
roof(52, 24, 6, 12)
# flag on the tall tower
for y in range(1, 8):
    px(26, y, 'stone')
for k, w in enumerate([5, 4, 4, 3]):
    for x in range(27, 27 + w):
        px(x, 1 + k, 'flag')
# gate
for y in range(72, 84):
    for x in range(48, 57):
        if y > 73 or 49 <= x <= 55:
            px(x, y, 'walldk' if (x - 48) % 2 else 'rockdk')
for x in range(47, 58):
    px(x, 71, 'stonehi')
# lit windows
for (x, y) in [(25, 34), (25, 46), (25, 58), (43, 44), (51, 44), (60, 44), (43, 54), (60, 54),
               (52, 29), (75, 40), (75, 52), (19, 70), (32, 70), (84, 70)]:
    window(x, y, 3 if y < 60 else 2)
# path of stones up to the gate
for k in range(14):
    x, y = 56 + k * 3, 86 + k * 2
    px(x, y, 'stone')
    px(x + 1, y, 'stonehi')

# --- foreground: ground, tombstones, dead tree
for x in range(W):
    top = 121 + int(2 * np.sin(x / 9.0))
    for y in range(top, H):
        px(x, y, 'ground')
    if rng.random() < 0.35:
        px(x, top - 1, 'grass')
for gx in [8, 22, 64, 80]:
    rect(gx, 112, gx + 5, 122, 'stone')
    rect(gx + 1, 111, gx + 4, 112, 'stone')
    px(gx, 112, 'stonehi')
    px(gx + 2, 114, 'rockdk')
    px(gx + 1, 115, 'rockdk')
    px(gx + 3, 115, 'rockdk')
# dead tree (left edge)
for y in range(70, 122):
    px(3, y, 'rockdk')
    px(4, y, 'rockdk')
for (x0, y0, dx, n_) in [(4, 86, 1, 9), (3, 94, -1, 4), (4, 78, 1, 6), (5, 90, 1, 5)]:
    for k in range(n_):
        px(x0 + dx * k, y0 - k // 2, 'rockdk')

Image.fromarray(img).save('assets/castle.png')
print('wrote assets/castle.png', img.shape)

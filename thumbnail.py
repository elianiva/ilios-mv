# Thumbnail from a still of the title over the magic circle, rendered without the letterbox and
# at 1.5x so the crop has room:
#   ILIOS_NO_BARS=1 ./target/release/ilios --scale 1.5 frame 22.2s
#   python3 thumbnail.py frames/22.2s.png thumbnail
# Writes thumbnail.png (1920x1080) and thumbnail-1280.jpg (1280x720, for YouTube).
#
# The circle is softened and faded into the dark; the title and the names stay crisp and
# bright on a pool of shade, so they read at the small size YouTube shows thumbnails at. The
# lettering (title, ILIOS and the names) is placed at the optical centre, a little above the
# middle: centred by the numbers, the block looks like it sags.

import sys, numpy as np
from PIL import Image, ImageFilter

OPTICAL_Y = 0.46  # where the middle of the lettering's block goes, as a fraction of the height

src, out = sys.argv[1], sys.argv[2]
full = Image.open(src).convert('RGB')


def blur(x, r):
    return np.asarray(Image.fromarray((np.clip(x, 0, 1) * 255).astype(np.uint8)).filter(ImageFilter.GaussianBlur(r))).astype(np.float32) / 255


def lettering(a):
    """The lettering: bright and nearly colourless. The ring's lines are thin and dim, so a
    blur-then-threshold keeps the letters and drops the lines."""
    lum = a @ np.array([0.299, 0.587, 0.114], np.float32)
    sat = a.max(2) - a.min(2)
    return ((lum > 0.55) & (sat < 0.22)).astype(np.float32)


# Find the lettering in the whole still and crop 16:9 round it, as large as fits, with the
# lettering's middle at the optical centre.
fa = np.asarray(full).astype(np.float32) / 255
# Rows with real lettering in them; then the block round the title, bridging the gaps between
# title, ILIOS and the names but not out to the small glyphs on the ring.
rows = np.nonzero(lettering(fa).sum(1) >= 40)[0]
peak = int(np.argmax(lettering(fa).sum(1)))
lo = hi = peak
gap = 0.11 * fa.shape[0]
for y in rows[rows > peak]:
    if y - hi > gap:
        break
    hi = y
for y in rows[rows < peak][::-1]:
    if lo - y > gap:
        break
    lo = y
ys = np.array([lo, hi])
W, H = full.size
mid = 0.5 * (ys.min() + ys.max())
# As wide as the title plus a margin, so it is big but never clipped.
cols = np.nonzero(lettering(fa)[ys.min():ys.max()].sum(0) >= 40)[0]
cw = min(W, 1.35 * (cols.max() - cols.min()))
ch = cw * 9 / 16
x0 = min(max(0.5 * (cols.min() + cols.max()) - cw / 2, 0), W - cw)
y0 = mid - OPTICAL_Y * ch
print(f'lettering rows {ys.min()}..{ys.max()} of {H}; crop {cw:.0f}x{ch:.0f} at {x0:.0f},{y0:.0f}')
# The still ends below the names, so the crop can run past its bottom edge: the picture fades
# to the dark of space over its last rows and the rest is filled with that dark.
pad = max(0, int(np.ceil(y0 + ch - H)))
if pad:
    fade = np.clip((H - np.arange(H)) / (0.22 * H), 0, 1) ** 1.5
    dark = np.array([0.02, 0.016, 0.045], np.float32)
    fa = fa * fade[:, None, None] + dark * (1 - fade[:, None, None])
    fa = np.concatenate([fa, np.broadcast_to(dark, (pad, W, 3))], 0)
    full = Image.fromarray((fa * 255).astype(np.uint8))
im = full.crop((round(x0), round(y0), round(x0 + cw), round(y0 + ch))).resize((1920, 1080), Image.LANCZOS)
a = np.asarray(im).astype(np.float32) / 255

text = lettering(a)
mask = blur(np.clip(blur(text, 3) * 2.2, 0, 1), 1)

# Background: a gentle glow only, then a faded print with lifted violet blacks.
bg = 1 - (1 - a) * (1 - 0.18 * blur(a, 24))
bg = 0.05 + 0.85 * bg
bg = bg * np.array([0.98, 0.97, 1.03]) + np.array([0.010, 0.005, 0.025])
# A pool of shade round the lettering, so it stands off the ring.
shade = blur(mask, 22)
bg = bg * (1 - 0.55 * np.clip(shade * 1.6, 0, 1))[..., None]
# The edges of the circle fade into the dark, the centre stays clear.
yy, xx = np.mgrid[0:1080, 0:1920]
r = np.sqrt(((xx - 960) / 960) ** 2 + ((yy - 540) / 600) ** 2)
bg = bg * (1 - 0.55 * np.clip((r - 0.45) / 0.75, 0, 1) ** 1.6)[..., None]

# The lettering on top, crisp, a touch brighter, with a tight glow only.
letters = np.clip(a * 1.12 + 0.03, 0, 1)
tight = blur(letters * mask[..., None], 4)
out_img = bg * (1 - mask[..., None]) + letters * mask[..., None]
out_img = 1 - (1 - out_img) * (1 - 0.25 * tight)

img = Image.fromarray((np.clip(out_img, 0, 1) * 255).astype(np.uint8))
img.save(out + '.png')
img.resize((1280, 720), Image.LANCZOS).save(out + '-1280.jpg', quality=92, optimize=True, progressive=True)

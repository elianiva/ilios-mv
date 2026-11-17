# Thumbnail from a rendered frame of the title over the magic circle, e.g. 22.2 s:
#   ./target/release/ilios frame 22.2s && python3 thumbnail.py frames/22.2s.png thumbnail
# Writes thumbnail.png (1920x1080) and thumbnail-1280.jpg (1280x720, for YouTube).
#
# The circle is softened and faded into the dark; the title and the names stay crisp and
# bright on a pool of shade, so they read at the small size YouTube shows thumbnails at.

import sys, numpy as np
from PIL import Image, ImageFilter

src, out = sys.argv[1], sys.argv[2]
im = Image.open(src).convert('RGB')
# Inside the letterbox, cut to 16:9 around the ring.
top, bot = 92, 988
h = bot - top; w = round(h * 16 / 9); x0 = (1920 - w) // 2
im = im.crop((x0, top, x0 + w, bot)).resize((1920, 1080), Image.LANCZOS)
a = np.asarray(im).astype(np.float32) / 255


def blur(x, r):
    return np.asarray(Image.fromarray((np.clip(x, 0, 1) * 255).astype(np.uint8)).filter(ImageFilter.GaussianBlur(r))).astype(np.float32) / 255


# The lettering: bright and nearly colourless. The ring's lines are thin and dim, so a
# blur-then-threshold keeps the letters and drops the lines.
lum = a @ np.array([0.299, 0.587, 0.114], np.float32)
sat = a.max(2) - a.min(2)
text = ((lum > 0.55) & (sat < 0.22)).astype(np.float32)
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

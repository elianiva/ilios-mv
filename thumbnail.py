# Thumbnail from a rendered frame of the title over the magic circle, e.g. 22.2 s:
#   ./target/release/ilios frame 22.2s && python3 thumbnail.py frames/22.2s.png thumbnail
# Writes thumbnail.png (1920x1080) and thumbnail-1280.jpg (1280x720, for YouTube).

import sys, numpy as np
from PIL import Image, ImageFilter
src, out = sys.argv[1], sys.argv[2]
im = Image.open(src).convert('RGB')
# Inside the letterbox, cut to 16:9 around the ring.
top, bot = 92, 988
h = bot - top; w = round(h * 16 / 9); x0 = (1920 - w) // 2
im = im.crop((x0, top, x0 + w, bot)).resize((1920, 1080), Image.LANCZOS)
a = np.asarray(im).astype(np.float32) / 255
# Soft bloom: the light breathes off the lines and letters.
glow = np.asarray(im.filter(ImageFilter.GaussianBlur(28))).astype(np.float32) / 255
a = 1 - (1 - a) * (1 - 0.45 * glow)
# Faded print: lifted, cool-violet blacks and gentler contrast.
a = 0.06 + 0.88 * a
a = a * np.array([0.98, 0.97, 1.03]) + np.array([0.012, 0.006, 0.03])
# The edges of the circle fade into the dark, the centre stays clear.
yy, xx = np.mgrid[0:1080, 0:1920]
r = np.sqrt(((xx - 960) / 960) ** 2 + ((yy - 540) / 600) ** 2)
fade = 1 - 0.55 * np.clip((r - 0.45) / 0.75, 0, 1) ** 1.6
a = a * fade[..., None]
img = Image.fromarray((np.clip(a, 0, 1) * 255).astype(np.uint8))
img.save(out + '.png')
img.resize((1280, 720), Image.LANCZOS).save(out + '-1280.jpg', quality=92, optimize=True, progressive=True)

#!/usr/bin/env python3
"""Draws LMS Shell's placeholder art: the companion's poses (ui/assets/mascot/*.png), the small UI icon
(ui/assets/icon.png), the logos (ui/assets/logo*.svg) and the 1024 px app icon (src-tauri/app-icon.png).

Everything is a plain geometric shape so a fork can swap it for real art: keep the file names, replace
the files, rebuild. See README "Rebrand in 5 steps". Needs Pillow (pip install pillow).

    python3 tools/make-placeholder-art.py            # writes the files above
    cd src-tauri && cargo tauri icon app-icon.png    # then regenerates src-tauri/icons/*
"""
import math
import pathlib

from PIL import Image, ImageDraw

ROOT = pathlib.Path(__file__).resolve().parent.parent
BRAND = (15, 118, 110)      # keep in step with --brand in ui/style.css
BRAND_DARK = (11, 80, 75)
WHITE = (255, 255, 255)
INK = (20, 23, 29)
GOLD = (217, 119, 6)
S = 512                      # pose canvas
K = 4                        # supersampling for smooth edges

POSES = {
    # name: (eyes dx, eyes dy, eye shape, mouth, arms, tilt, prop)
    "idle": (0, 0, "open", "smile", None, 0, None),
    "wave": (0, 0, "open", "grin", "wave", 0, None),
    "thumbsup": (0, 0, "happy", "grin", "up-right", 0, None),
    "celebrate": (0, -6, "happy", "open", "both-up", 0, "spark"),
    "sleepy": (0, 6, "closed", "flat", None, -6, "zzz"),
    "reading": (0, 10, "open", "flat", "front", 0, "book"),
    "thinking": (8, -10, "open", "flat", "chin", 4, None),
    "pointing": (14, 0, "open", "smile", "right", 0, None),
    "walk": (10, 0, "open", "smile", "swing", 4, None),
    "hop": (0, -4, "happy", "open", "both-up", 0, None),
    "point-left": (-16, 0, "open", "smile", "left", 0, None),
    "point-right": (16, 0, "open", "smile", "right", 0, None),
    "point-up": (0, -16, "open", "smile", "up", 0, None),
    "point-down": (0, 14, "open", "smile", "down", 0, None),
    "peek": (12, 0, "open", "smile", None, 10, None),
    "highfive": (0, 0, "happy", "grin", "up-right", 0, None),
    "map": (0, 8, "open", "smile", "front", 0, "map"),
    "flag": (0, 0, "happy", "grin", "up-right", 0, "flag"),
    "trophy": (0, 0, "happy", "grin", "front", 0, "trophy"),
    "encourage": (0, 0, "open", "grin", "both-up", 0, None),
    "idea": (0, -12, "open", "open", "up", 0, "bulb"),
    "goodbye": (0, 0, "happy", "smile", "wave", -4, None),
    "fly": (0, 0, "open", "open", "both-up", -12, "spark"),
    "takeoff": (0, -10, "open", "open", "both-up", 0, None),
    "landing": (0, 10, "open", "smile", "both-up", 0, None),
    "dizzy": (0, 0, "x", "wave", None, 12, "spark"),
    "laugh": (0, 0, "happy", "open", None, 6, None),
    "salute": (0, 0, "open", "smile", "salute", 0, None),
    "wink": (0, 0, "wink", "grin", None, 4, None),
    "oops": (0, 0, "open", "o", "chin", -6, None),
    "mood-low": (0, 10, "closed", "frown", None, -4, None),
    "view-back": (0, 0, "none", "none", None, 0, None),
    "view-side-left": (-26, 0, "open", "smile", None, 0, None),
    "view-side-right": (26, 0, "open", "smile", None, 0, None),
    "view-three-quarter-left": (-14, 0, "open", "smile", None, 0, None),
    "view-three-quarter-right": (14, 0, "open", "smile", None, 0, None),
}


def pose(name, spec):
    dx, dy, eyes, mouth, arms, tilt, prop = spec
    z = S * K
    im = Image.new("RGBA", (z, z), (0, 0, 0, 0))
    d = ImageDraw.Draw(im)
    u = lambda v: int(v * K)
    cx, cy, r = 256, 280, 150
    # arms: small circles at the body's edge
    spots = {
        "wave": [(400, 150)], "up-right": [(410, 190)], "both-up": [(108, 150), (404, 150)], "front": [(200, 380), (312, 380)],
        "chin": [(300, 340)], "right": [(440, 280)], "left": [(72, 280)], "up": [(330, 110)], "down": [(330, 450)],
        "swing": [(120, 330), (392, 250)], "salute": [(352, 168)],
    }.get(arms, [])
    for (ax, ay) in spots:
        d.ellipse([u(ax - 28), u(ay - 28), u(ax + 28), u(ay + 28)], fill=BRAND_DARK)
    # body: a rounded square
    d.rounded_rectangle([u(cx - r), u(cy - r), u(cx + r), u(cy + r)], radius=u(70), fill=BRAND)
    d.rounded_rectangle([u(cx - r + 18), u(cy - r + 14), u(cx + r - 60), u(cy - r + 40)], radius=u(13), fill=(46, 150, 141))
    # antenna spark
    d.line([u(cx), u(cy - r), u(cx), u(cy - r - 40)], fill=BRAND_DARK, width=u(10))
    d.ellipse([u(cx - 18), u(cy - r - 62), u(cx + 18), u(cy - r - 26)], fill=GOLD)
    ex, ey = cx + dx, cy - 30 + dy
    for i, side in enumerate((-1, 1)):
        x = ex + side * 52
        if eyes == "none":
            continue
        if eyes == "open" or (eyes == "wink" and side == -1):
            d.ellipse([u(x - 26), u(ey - 30), u(x + 26), u(ey + 30)], fill=WHITE)
            d.ellipse([u(x - 11 + dx * 0.3), u(ey - 12 + dy * 0.3), u(x + 11 + dx * 0.3), u(ey + 12 + dy * 0.3)], fill=INK)
        elif eyes in ("happy", "wink"):
            d.arc([u(x - 26), u(ey - 14), u(x + 26), u(ey + 30)], 200, 340, fill=WHITE, width=u(10))
        elif eyes == "closed":
            d.line([u(x - 24), u(ey + 6), u(x + 24), u(ey + 6)], fill=WHITE, width=u(10))
        elif eyes == "x":
            for a, b in (((-18, -18), (18, 18)), ((-18, 18), (18, -18))):
                d.line([u(x + a[0]), u(ey + a[1]), u(x + b[0]), u(ey + b[1])], fill=WHITE, width=u(10))
    mx, my = cx + dx * 0.8, cy + 62 + dy * 0.5
    if mouth == "smile":
        d.arc([u(mx - 46), u(my - 34), u(mx + 46), u(my + 22)], 20, 160, fill=WHITE, width=u(10))
    elif mouth == "grin":
        d.chord([u(mx - 50), u(my - 30), u(mx + 50), u(my + 34)], 0, 180, fill=WHITE)
    elif mouth == "open":
        d.ellipse([u(mx - 24), u(my - 18), u(mx + 24), u(my + 28)], fill=WHITE)
    elif mouth == "o":
        d.ellipse([u(mx - 14), u(my - 10), u(mx + 14), u(my + 18)], fill=WHITE)
    elif mouth == "flat":
        d.line([u(mx - 30), u(my + 4), u(mx + 30), u(my + 4)], fill=WHITE, width=u(10))
    elif mouth == "frown":
        d.arc([u(mx - 40), u(my), u(mx + 40), u(my + 50)], 200, 340, fill=WHITE, width=u(10))
    elif mouth == "wave":
        pts = [(u(mx - 40 + i * 8), u(my + 6 + 8 * math.sin(i))) for i in range(11)]
        d.line(pts, fill=WHITE, width=u(8))
    # props
    if prop == "spark":
        for (sx, sy) in ((96, 96), (420, 80), (450, 420)):
            d.regular_polygon((u(sx), u(sy), u(18)), 4, rotation=45, fill=GOLD)
    elif prop == "zzz":
        for i, (zx, zy) in enumerate(((380, 120), (420, 80), (452, 44))):
            s_ = 16 + i * 6
            d.line([u(zx - s_), u(zy - s_), u(zx + s_), u(zy - s_), u(zx - s_), u(zy + s_), u(zx + s_), u(zy + s_)], fill=BRAND_DARK, width=u(6))
    elif prop == "book":
        d.rectangle([u(170), u(392), u(342), u(470)], fill=WHITE, outline=BRAND_DARK, width=u(6))
        d.line([u(256), u(392), u(256), u(470)], fill=BRAND_DARK, width=u(6))
    elif prop == "map":
        d.polygon([u(160), u(400), u(220), u(386), u(290), u(404), u(352), u(388), u(352), u(470), u(290), u(486), u(220), u(468), u(160), u(484)], fill=(254, 243, 199), outline=BRAND_DARK)
    elif prop == "flag":
        d.line([u(410), u(190), u(410), u(40)], fill=INK, width=u(8))
        d.polygon([u(410), u(40), u(490), u(66), u(410), u(92)], fill=GOLD)
    elif prop == "trophy":
        d.rectangle([u(226), u(470), u(286), u(492)], fill=GOLD)
        d.pieslice([u(196), u(352), u(316), u(472)], 0, 180, fill=GOLD)
        d.rectangle([u(196), u(352), u(316), u(412)], fill=GOLD)
    elif prop == "bulb":
        d.ellipse([u(392), u(40), u(462), u(110)], fill=(253, 224, 71))
        d.rectangle([u(412), u(108), u(442), u(128)], fill=INK)
    im = im.rotate(tilt, resample=Image.BICUBIC, center=(z / 2, z / 2))
    return im.resize((S, S), Image.LANCZOS)


def badge(size):
    """The app icon: the companion's face on a rounded tile."""
    z = size * K
    im = Image.new("RGBA", (z, z), (0, 0, 0, 0))
    d = ImageDraw.Draw(im)
    m = z * 0.06
    d.rounded_rectangle([m, m, z - m, z - m], radius=z * 0.22, fill=BRAND)
    for side in (-1, 1):
        x = z / 2 + side * z * 0.15
        d.ellipse([x - z * 0.07, z * 0.36, x + z * 0.07, z * 0.52], fill=WHITE)
    d.arc([z * 0.34, z * 0.5, z * 0.66, z * 0.7], 20, 160, fill=WHITE, width=int(z * 0.035))
    d.ellipse([z * 0.46, z * 0.12, z * 0.54, z * 0.2], fill=GOLD)
    return im.resize((size, size), Image.LANCZOS)


LOGO = """<svg xmlns="http://www.w3.org/2000/svg" width="132" height="28" viewBox="0 0 132 28" role="img" aria-label="LMS Shell">
  <rect x="1" y="2" width="24" height="24" rx="6" fill="#0f766e"/>
  <circle cx="9.5" cy="12.5" r="2.4" fill="#fff"/><circle cx="16.5" cy="12.5" r="2.4" fill="#fff"/>
  <path d="M8.5 17.5 Q13 21 17.5 17.5" stroke="#fff" stroke-width="1.8" fill="none" stroke-linecap="round"/>
  <text x="32" y="19.5" font-family="'Plus Jakarta Sans', -apple-system, 'Segoe UI', sans-serif" font-size="16" font-weight="700" fill="{ink}">LMS Shell</text>
</svg>
"""


def main():
    out = ROOT / "ui/assets/mascot"
    out.mkdir(parents=True, exist_ok=True)
    for name, spec in POSES.items():
        pose(name, spec).save(out / f"{name}.png", optimize=True)
    badge(96).save(ROOT / "ui/assets/icon.png", optimize=True)
    badge(1024).save(ROOT / "src-tauri/app-icon.png", optimize=True)
    (ROOT / "ui/assets/logo.svg").write_text(LOGO.replace("{ink}", "#14171d"))
    (ROOT / "ui/assets/logo-white.svg").write_text(LOGO.replace("{ink}", "#eceef3"))
    print(f"wrote {len(POSES)} poses, icon.png, app-icon.png and two logos")


if __name__ == "__main__":
    main()

#!/usr/bin/env python3
"""Rasterize the sidebar brand mark (M on the conic gradient) to a square PNG.

Writes apps/desktop/app-icon.png (source) and src-tauri/icons/icon.png.
Then: cd apps/desktop && pnpm tauri icon app-icon.png
Keep only the desktop files listed in tauri.conf.json (drop android/ios/Appx).
"""
from __future__ import annotations

import math
from pathlib import Path

from PIL import Image, ImageDraw, ImageFilter, ImageFont

SIZE = 1024
# Match .mark in App.svelte: 36px box, 11px radius.
RADIUS = round(SIZE * 11 / 36)
FONT = "/usr/share/fonts/rsms-inter-fonts/Inter-ExtraBold.ttf"
STOPS = (
    (0.0, (244, 114, 182)),  # #f472b6
    (1 / 3, (139, 92, 246)),  # #8b5cf6
    (2 / 3, (34, 211, 238)),  # #22d3ee
    (1.0, (244, 114, 182)),
)
# CSS conic-gradient(from 200deg, …): 0deg is 12 o'clock, clockwise.
FROM_DEG = 200.0


def lerp(a: tuple[int, int, int], b: tuple[int, int, int], t: float) -> tuple[int, int, int]:
    return tuple(int(a[i] + (b[i] - a[i]) * t) for i in range(3))  # type: ignore[return-value]


def conic(css_deg: float) -> tuple[int, int, int]:
    t = ((css_deg - FROM_DEG) % 360.0) / 360.0
    for i in range(len(STOPS) - 1):
        t0, c0 = STOPS[i]
        t1, c1 = STOPS[i + 1]
        if t <= t1:
            u = 0.0 if t1 == t0 else (t - t0) / (t1 - t0)
            return lerp(c0, c1, u)
    return STOPS[-1][1]


def rounded_mask(size: int, radius: int) -> Image.Image:
    mask = Image.new("L", (size, size), 0)
    ImageDraw.Draw(mask).rounded_rectangle((0, 0, size - 1, size - 1), radius=radius, fill=255)
    return mask


def paint_conic(size: int) -> Image.Image:
    cx = cy = (size - 1) / 2
    px = bytearray(size * size * 3)
    i = 0
    for y in range(size):
        dy = y - cy
        for x in range(size):
            dx = x - cx
            # atan2(dx, -dy): 0° at top, clockwise — CSS angles.
            css_deg = math.degrees(math.atan2(dx, -dy)) % 360.0
            r, g, b = conic(css_deg)
            px[i] = r
            px[i + 1] = g
            px[i + 2] = b
            i += 3
    return Image.frombytes("RGB", (size, size), bytes(px))


def draw_m(img: Image.Image) -> None:
    font = ImageFont.truetype(FONT, size=round(SIZE * 0.52))
    draw = ImageDraw.Draw(img)
    letter = "M"
    bbox = draw.textbbox((0, 0), letter, font=font)
    w, h = bbox[2] - bbox[0], bbox[3] - bbox[1]
    x = (SIZE - w) / 2 - bbox[0]
    y = (SIZE - h) / 2 - bbox[1] - SIZE * 0.02
    draw.text((x, y), letter, font=font, fill=(255, 255, 255, 255))


def main() -> None:
    root = Path(__file__).resolve().parents[1]
    out_src = root / "apps/desktop/app-icon.png"
    out_icon = root / "apps/desktop/src-tauri/icons/icon.png"
    out_icon.parent.mkdir(parents=True, exist_ok=True)

    fill = paint_conic(SIZE).convert("RGBA")
    mask = rounded_mask(SIZE, RADIUS)
    fill.putalpha(mask)

    # Soft violet drop shadow, same idea as the sidebar mark.
    shadow = Image.new("RGBA", (SIZE, SIZE), (0, 0, 0, 0))
    ImageDraw.Draw(shadow).rounded_rectangle(
        (0, 0, SIZE - 1, SIZE - 1), radius=RADIUS, fill=(139, 92, 246, 180)
    )
    shadow = shadow.filter(ImageFilter.GaussianBlur(radius=SIZE // 28))
    canvas = Image.new("RGBA", (SIZE, SIZE), (0, 0, 0, 0))
    canvas.alpha_composite(shadow, (0, SIZE // 48))
    canvas.alpha_composite(fill)
    draw_m(canvas)

    canvas.save(out_src, "PNG")
    canvas.save(out_icon, "PNG")
    print(f"wrote {out_src.relative_to(root)}")
    print(f"wrote {out_icon.relative_to(root)}")


if __name__ == "__main__":
    main()

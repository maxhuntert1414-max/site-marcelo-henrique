"""Gera os ícones do Teclado Remoto (Windows .ico e prévia .png).

Uso: python3 tools/make_icons.py   (precisa de Pillow)
"""
from pathlib import Path

from PIL import Image, ImageDraw

ROOT = Path(__file__).resolve().parent.parent
RES = ROOT / "pc-windows" / "res"
SIZES = [16, 20, 24, 32, 40, 48, 64, 96, 128, 256]
SUPERSAMPLE = 8

PALETTES = {
    "icon.ico": ((59, 130, 246), (29, 78, 216)),
    "icon-idle.ico": ((148, 163, 184), (100, 116, 139)),
}


def draw(size, top, bottom):
    s = size * SUPERSAMPLE
    img = Image.new("RGBA", (s, s), (0, 0, 0, 0))
    grad = Image.new("RGBA", (s, s))
    gd = ImageDraw.Draw(grad)
    for y in range(s):
        t = y / (s - 1)
        gd.line([(0, y), (s, y)], fill=tuple(round(a + (b - a) * t) for a, b in zip(top, bottom)) + (255,))
    mask = Image.new("L", (s, s), 0)
    ImageDraw.Draw(mask).rounded_rectangle([0, 0, s - 1, s - 1], radius=round(s * 0.22), fill=255)
    img.paste(grad, (0, 0), mask)

    d = ImageDraw.Draw(img)
    u = s / 100
    d.rounded_rectangle([12 * u, 27 * u, 88 * u, 75 * u], radius=9 * u, fill=(255, 255, 255, 255))
    key = bottom + (255,)
    cols, gap, left, right = 5, 3.2, 18, 82
    width = (right - left - gap * (cols - 1)) / cols
    for row, y in enumerate((34, 47)):
        for c in range(cols):
            x = left + c * (width + gap)
            d.rounded_rectangle([x * u, y * u, (x + width) * u, (y + 9.5) * u], radius=2.2 * u, fill=key)
    d.rounded_rectangle([30 * u, 61 * u, 70 * u, 68 * u], radius=2.2 * u, fill=key)
    return img.resize((size, size), Image.LANCZOS)


def main():
    RES.mkdir(parents=True, exist_ok=True)
    for name, (top, bottom) in PALETTES.items():
        frames = [draw(size, top, bottom) for size in SIZES]
        frames[-1].save(RES / name, format="ICO", sizes=[(f.width, f.height) for f in frames], append_images=frames[:-1])
    draw(512, *PALETTES["icon.ico"]).save(ROOT / "docs" / "icone.png")


if __name__ == "__main__":
    main()

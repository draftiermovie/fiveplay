"""Builds the Windows icon and installer artwork from packaging/icon.png.

    python tools/make_icons.py

Writes, next to the Windows installer source (packaging/windows/):
  fiveplay.ico  every size Windows asks for (16-256 px); embedded in
                FivePlay.exe by build.rs and shown by the installer
  banner.bmp    493x58 strip across the top of the installer's pages
  dialog.bmp    493x312 background of its welcome and finish pages
Needs Pillow (pip install pillow). Run it again after changing the icon.
"""
from pathlib import Path

from PIL import Image, ImageDraw, ImageFont

ROOT = Path(__file__).resolve().parent.parent
ICON = ROOT / "packaging" / "icon.png"
OUT = ROOT / "packaging" / "windows"

NAVY = (11, 18, 28)
SKY = (98, 200, 255)


def font(size):
    """A bold sans font if the system has one, else Pillow's default."""
    for name in ("DejaVuSans-Bold.ttf", "arialbd.ttf", "Arial Bold.ttf"):
        try:
            return ImageFont.truetype(name, size)
        except OSError:
            pass
    return ImageFont.load_default(size)


def main():
    icon = Image.open(ICON).convert("RGBA")
    OUT.mkdir(parents=True, exist_ok=True)

    sizes = [(s, s) for s in (16, 20, 24, 32, 40, 48, 64, 128, 256)]
    icon.save(OUT / "fiveplay.ico", sizes=sizes)

    # Banner: white for the page title on the left, the icon on the right.
    banner = Image.new("RGB", (493, 58), "white")
    small = icon.resize((46, 46), Image.LANCZOS)
    banner.paste(small, (493 - 46 - 8, 6), small)
    banner.save(OUT / "banner.bmp")

    # Dialog: the left 164 px are artwork, the rest stays white for text.
    dialog = Image.new("RGB", (493, 312), "white")
    draw = ImageDraw.Draw(dialog)
    for y in range(312):
        t = y / 311
        shade = tuple(int(c * (1 - t) + d * t) for c, d in zip((22, 34, 50), NAVY))
        draw.line([(0, y), (163, y)], fill=shade)
    big = icon.resize((112, 112), Image.LANCZOS)
    dialog.paste(big, (26, 70), big)
    title = font(20)
    width = draw.textlength("FIVEPLAY", font=title)
    draw.text(((164 - width) / 2, 200), "FIVEPLAY", fill=SKY, font=title)
    tag = font(10)
    width = draw.textlength("ARENA SHOOTER", font=tag)
    draw.text(((164 - width) / 2, 228), "ARENA SHOOTER", fill=(150, 170, 190), font=tag)
    dialog.save(OUT / "dialog.bmp")
    print("wrote", ", ".join(p.name for p in sorted(OUT.glob("*.*"))))


if __name__ == "__main__":
    main()

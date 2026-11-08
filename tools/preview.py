"""Render a contact sheet of every mascot's animations to a PNG -- no window needed.

    python tools/preview.py [out.png] [mascot ...]
"""
from __future__ import annotations

import os
import random
import sys
from pathlib import Path

os.environ.setdefault("QT_QPA_PLATFORM", "offscreen")
sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from PySide6.QtCore import QRectF, Qt  # noqa: E402
from PySide6.QtGui import QColor, QFont, QGuiApplication, QImage, QPainter  # noqa: E402

from desktop_mascots.characters import CHARACTERS  # noqa: E402
from desktop_mascots.engine import Bounds  # noqa: E402

FLOOR = 1000.0
BOUNDS = Bounds(left=0, right=2000, top=0, floor=FLOOR)
DT = 1 / 60
CURSOR = (250, -60)   # up and to the right: idle mascots look there, chasers run there


def _brain(char):
    b = char.brain(1000, FLOOR, rng=random.Random(3))
    b.update(DT, BOUNDS)  # lands immediately
    b.go("idle", 99)
    return b


def _step(b, seconds):
    for _ in range(max(1, round(seconds / DT))):
        b.update(DT, BOUNDS, cursor=CURSOR)
    return b


def scene(char, state, duration, at, facing=1):
    b = _brain(char)
    b.facing = facing
    b.go(state, duration)
    return _step(b, at)


def dragged(char):
    b = _brain(char)
    b.grab()
    for i in range(20):
        b.drag_to(1000 + i * 15, FLOOR - 200)
        b.update(DT, BOUNDS)
    return b


def poked(char):
    b = _brain(char)
    b.say(char.brain.LINES[0])
    return _step(b, 0.1)


def said(char):
    b = _brain(char)
    b.say("Your build finished in 3m12s with 0 errors and 2 warnings. Time for a banana break?", 9,
          important=True)
    return _step(b, 0.1)


def thrown(char):
    b = _brain(char)
    b.y = FLOOR - 400
    b.vx, b.vy = 300, -100
    b._air_kind = "fall"
    b.go("air")
    return _step(b, 0.1)


def scenes(char):
    yield "idle (eyes on cursor)", lambda: scene(char, "idle", 99, 0.5)
    yield "walk", lambda: scene(char, "walk", 99, 0.17)
    yield "walk (left)", lambda: scene(char, "walk", 99, 0.52, facing=-1)
    for label, state, duration, at in char.previews:
        yield label, lambda s=state, d=duration, a=at: scene(char, s, d, a)
    yield "crouch", lambda: scene(char, "crouch", 0.14, 0.12)
    yield "jump", lambda: _step(scene(char, "crouch", 0.14, 0.15), 0.08)
    yield "falling", lambda: thrown(char)
    yield "dragged", lambda: dragged(char)
    yield "land", lambda: scene(char, "land", 0.22, 0.03)
    yield "dizzy", lambda: scene(char, "dizzy", 2.6, 1.0)
    yield "poked", lambda: poked(char)
    yield "say (sent text)", lambda: said(char)


def main() -> None:
    out = Path(sys.argv[1] if len(sys.argv) > 1 else "preview.png")
    chars = [CHARACTERS[n] for n in sys.argv[2:]] or list(CHARACTERS.values())
    QGuiApplication(sys.argv[:1])
    cols, cw, ch, s = 5, 250, 250, 1.4
    sections = [(c, list(scenes(c))) for c in chars]
    rows = sum((len(sc) + cols - 1) // cols for _, sc in sections)
    title_h = 28
    img = QImage(cols * cw, rows * ch + len(sections) * title_h, QImage.Format_ARGB32_Premultiplied)
    img.fill(QColor("#DDE6EE"))
    p = QPainter(img)
    font, title = QFont(), QFont()
    font.setPixelSize(12)
    title.setPixelSize(16)
    title.setBold(True)
    y0 = 0
    for char, items in sections:
        p.setPen(QColor("#223"))
        p.setFont(title)
        p.drawText(QRectF(10, y0, cols * cw, title_h), Qt.AlignVCenter, char.name.capitalize())
        y0 += title_h
        for i, (label, make) in enumerate(items):
            b = make()
            x, y = (i % cols) * cw, y0 + (i // cols) * ch
            p.save()
            p.translate(x + cw / 2, y + ch - 30 - (FLOOR - b.y) * 0.15)
            p.scale(s, s)
            char.paint(p, b.pose, b.facing)
            p.restore()
            p.setPen(QColor("#334"))
            p.setFont(font)
            p.drawText(QRectF(x, y + ch - 24, cw, 20), Qt.AlignCenter, f"{label}  [{b.state}]")
        y0 += ((len(items) + cols - 1) // cols) * ch
    p.end()
    img.save(str(out))
    print(f"wrote {out.resolve()}")


if __name__ == "__main__":
    main()

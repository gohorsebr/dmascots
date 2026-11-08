"""Render a contact sheet of the monkey's animations to a PNG -- no window needed.

    python tools/preview.py [out.png]
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

from monkey_mascot.brain import Bounds, Brain  # noqa: E402
from monkey_mascot.monkey import paint_monkey  # noqa: E402

FLOOR = 1000.0
BOUNDS = Bounds(left=0, right=2000, top=0, floor=FLOOR)
DT = 1 / 60


def _brain() -> Brain:
    b = Brain(1000, FLOOR, rng=random.Random(3))
    b.update(DT, BOUNDS)  # lands immediately
    b._go_idle()
    return b


def _step(b: Brain, seconds: float, **kw) -> Brain:
    for _ in range(max(1, round(seconds / DT))):
        b.update(DT, BOUNDS, **kw)
    return b


def scene(state: str, dur: float, at: float, facing: int = 1, **kw) -> Brain:
    b = _brain()
    b.facing = facing
    b._go(state, dur)
    if state == "eat":
        b._dur = dur
    return _step(b, at, **kw)


def dragged() -> Brain:
    b = _brain()
    b.grab()
    for i in range(20):
        b.drag_to(1000 + i * 15, FLOOR - 200)
        b.update(DT, BOUNDS)
    return b


def poked() -> Brain:
    b = _brain()
    b._go("idle", 99)
    b.say("Ook ook!")
    return _step(b, 0.1)


def thrown() -> Brain:
    b = _brain()
    b.y = FLOOR - 400
    b.vx, b.vy = 300, -100
    b._air_kind = "fall"
    b._go("air")
    return _step(b, 0.1)


SCENES = [
    ("idle (looking at cursor)", lambda: scene("idle", 99, 0.5, cursor=(120, -60))),
    ("walk", lambda: scene("walk", 99, 0.17)),
    ("walk (left)", lambda: scene("walk", 99, 0.52, facing=-1)),
    ("scratch", lambda: scene("scratch", 3.0, 1.0)),
    ("eat: bite", lambda: scene("eat", 5.2, 1.2)),
    ("eat: chew", lambda: scene("eat", 5.2, 2.7)),
    ("eat: done", lambda: scene("eat", 5.2, 4.5)),
    ("crouch", lambda: scene("crouch", 0.14, 0.12)),
    ("jump", lambda: _step(scene("crouch", 0.14, 0.15), 0.08)),
    ("falling", thrown),
    ("dragged", dragged),
    ("land", lambda: scene("land", 0.22, 0.03)),
    ("dizzy", lambda: scene("dizzy", 2.6, 1.0)),
    ("poked", poked),
]


def main() -> None:
    out = Path(sys.argv[1] if len(sys.argv) > 1 else "preview.png")
    QGuiApplication(sys.argv[:1])
    cols, cw, ch, s = 5, 200, 220, 1.5
    rows = (len(SCENES) + cols - 1) // cols
    img = QImage(cols * cw, rows * ch, QImage.Format_ARGB32_Premultiplied)
    img.fill(QColor("#DDE6EE"))
    p = QPainter(img)
    font = QFont()
    font.setPixelSize(13)
    for i, (label, make) in enumerate(SCENES):
        b = make()
        x, y = (i % cols) * cw, (i // cols) * ch
        p.save()
        p.translate(x + cw / 2, y + ch - 30 - (FLOOR - b.y) * 0.15)
        p.scale(s, s)
        paint_monkey(p, b.pose, b.facing)
        p.restore()
        p.setPen(QColor("#334"))
        p.setFont(font)
        p.drawText(QRectF(x, y + ch - 24, cw, 20), Qt.AlignCenter, f"{label}  [{b.state}]")
    p.end()
    img.save(str(out))
    print(f"wrote {out.resolve()}")


if __name__ == "__main__":
    main()

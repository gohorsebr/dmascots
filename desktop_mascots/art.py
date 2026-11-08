"""Painting helpers shared by every character."""
from __future__ import annotations

import math
from functools import lru_cache

from PySide6.QtCore import QPointF, QRectF, Qt
from PySide6.QtGui import QColor, QFont, QFontMetricsF, QPainter, QPainterPath, QPen, QPixmap, QPolygonF

OUTLINE = QColor("#3A2314")
WHITE = QColor("#FFFFFF")
STAR = QColor("#FFD84A")
STAR_EDGE = QColor("#A9800F")
LINE_W = 1.6


def pen(color: QColor, width: float) -> QPen:
    p = QPen(color, width)
    p.setCapStyle(Qt.RoundCap)
    p.setJoinStyle(Qt.RoundJoin)
    return p


def stroke(p: QPainter, path: QPainterPath, color: QColor, width: float,
           outline: QColor = OUTLINE, outline_w: float = LINE_W) -> None:
    """A thick stroke with an outline (limbs, tails, necks)."""
    p.setBrush(Qt.NoBrush)
    p.setPen(pen(outline, width + 2 * outline_w))
    p.drawPath(path)
    p.setPen(pen(color, width))
    p.drawPath(path)


def blob(p: QPainter, c: QPointF, rx: float, ry: float, fill: QColor,
         outline: QColor = OUTLINE, outline_w: float = LINE_W) -> None:
    p.setPen(pen(outline, outline_w))
    p.setBrush(fill)
    p.drawEllipse(c, rx, ry)


def polyline(*pts: tuple[float, float]) -> QPainterPath:
    path = QPainterPath(QPointF(*pts[0]))
    for pt in pts[1:]:
        path.lineTo(QPointF(*pt))
    return path


def paint_stars(p: QPainter, cx: float, cy: float, angle: float, rx: float = 25, ry: float = 5) -> None:
    for i in range(3):
        a = angle + i * 2 * math.pi / 3
        c = QPointF(cx + rx * math.cos(a), cy + ry * math.sin(a))
        pts = []
        for k in range(10):
            r = 4.6 if k % 2 == 0 else 2.0
            t = -math.pi / 2 + k * math.pi / 5 + a * 2
            pts.append(QPointF(c.x() + r * math.cos(t), c.y() + r * math.sin(t)))
        p.setPen(pen(STAR_EDGE, 1.0))
        p.setBrush(STAR)
        p.drawPolygon(QPolygonF(pts))


BUBBLE_TEXT_W = 196.0     # wrap width
BUBBLE_LINES = 5          # longer messages are cut with an ellipsis


def _bubble_font() -> QFont:
    font = QFont()
    font.setBold(True)
    font.setPixelSize(11)
    return font


@lru_cache(maxsize=64)
def _bubble_layout(text: str) -> tuple[str, int, float, float]:
    """Wrap (and if needed truncate) text: returns (shown_text, flags, width, height)."""
    fm = QFontMetricsF(_bubble_font())
    max_h = fm.lineSpacing() * BUBBLE_LINES + 1

    def measure(t: str, flags: int) -> QRectF:
        return fm.boundingRect(QRectF(0, 0, BUBBLE_TEXT_W, 1e5), flags | int(Qt.AlignCenter), t)

    flags = int(Qt.TextWordWrap)
    if measure(text, flags).width() > BUBBLE_TEXT_W:   # a word too long to wrap (URL, path…)
        flags = int(Qt.TextWrapAnywhere)
    if measure(text, flags).height() > max_h:
        anywhere = flags == int(Qt.TextWrapAnywhere)
        parts = list(text) if anywhere else text.split(" ")
        joiner = "" if anywhere else " "
        while parts and measure(joiner.join(parts) + "…", flags).height() > max_h:
            parts.pop()
        text = joiner.join(parts).rstrip() + "…"
    r = measure(text, flags)
    return text, flags, min(r.width(), BUBBLE_TEXT_W), r.height()


def paint_bubble(p: QPainter, text: str, ax: float, ay: float, facing: int,
                 outline: QColor = OUTLINE, limit: float = 117.0) -> None:
    """Speech bubble whose tail points at (ax, ay); drawn unmirrored. Wraps long text."""
    p.save()
    p.setRenderHint(QPainter.Antialiasing, True)
    p.setFont(_bubble_font())
    text, flags, tw, th = _bubble_layout(text)
    w = tw + 16
    h = th + 9
    x = ax - 8 if facing > 0 else ax + 8 - w
    x = max(-limit, min(limit - w, x))
    rect = QRectF(x, ay - 8 - h, w, h)
    shape = QPainterPath()
    shape.addRoundedRect(rect, 8, 8)
    tail = polyline((ax - 4, rect.bottom() - 1), (ax, ay), (ax + 4, rect.bottom() - 1))
    tail.closeSubpath()
    p.setPen(pen(outline, 1.4))
    p.setBrush(WHITE)
    p.drawPath(shape.united(tail))
    p.setPen(outline)
    p.drawText(rect, flags | int(Qt.AlignCenter), text)
    p.restore()


def render_icon(character, size: int = 64) -> QPixmap:
    """The character's head, framed by its icon_box, for tray/menu icons."""
    cx, cy, extent = character.icon_box
    pm = QPixmap(size, size)
    pm.fill(Qt.transparent)
    p = QPainter(pm)
    p.setRenderHint(QPainter.Antialiasing, True)
    p.translate(size / 2, size / 2)
    p.scale(size / extent, size / extent)
    p.translate(-cx, -cy)
    character.paint(p, character.brain.pose_cls(), 1, head_only=True)
    p.end()
    return pm

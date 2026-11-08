"""Painting helpers shared by every character."""
from __future__ import annotations

import math

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


def paint_bubble(p: QPainter, text: str, ax: float, ay: float, facing: int,
                 outline: QColor = OUTLINE, limit: float = 82.0) -> None:
    """Speech bubble whose tail points at (ax, ay); drawn unmirrored."""
    p.save()
    p.setRenderHint(QPainter.Antialiasing, True)
    font = QFont()
    font.setBold(True)
    font.setPixelSize(11)
    p.setFont(font)
    w = QFontMetricsF(font).horizontalAdvance(text) + 14
    h = 20.0
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
    p.drawText(rect, Qt.AlignCenter, text)
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

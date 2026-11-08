"""Procedural monkey renderer: every frame is painted from a Pose, no image assets.

Coordinate system (before the caller's scale): origin between the feet,
y grows downward, the monkey faces +x. The painter is mirrored for facing=-1.
"""
from __future__ import annotations

import math

from PySide6.QtCore import QPointF, QRectF, Qt
from PySide6.QtGui import (QColor, QFont, QFontMetricsF, QPainter, QPainterPath,
                           QPen, QPixmap, QPolygonF)

from .pose import Pose

FUR = QColor("#8B5A2B")
FUR_SHADE = QColor("#72461F")
SKIN = QColor("#EECDA3")
OUTLINE = QColor("#3A2314")
MOUTH = QColor("#7A2424")
BANANA = QColor("#F6CE3A")
BANANA_EDGE = QColor("#A9800F")
BANANA_FLESH = QColor("#FFF4C9")
STAR = QColor("#FFD84A")
WHITE = QColor("#FFFFFF")

OUTLINE_W = 1.6
LIMB_W = 7.0

# Skeleton
HEAD_C = QPointF(0, -80)
HEAD_R = 22.0
NECK = QPointF(0, -60)
SCRUFF = QPointF(0, -96)        # pivot when dangling from the cursor
SHOULDER_X, SHOULDER_Y = 15.0, -56.0
HIP_X, HIP_Y = 9.0, -28.0
ARM_1 = ARM_2 = 15.0
LEG_1 = LEG_2 = 13.0
FACE_DX = 3.0                   # face shifted toward the facing side (3/4 look)

# Extents used by the window for layout / screen-edge collisions
HALF_WIDTH = 46.0               # feet → furthest tail/arm pixel
HEIGHT = 112.0                  # feet → top of head
HEAD_Y = 84.0                   # feet → eyes


def _pen(color: QColor, width: float) -> QPen:
    pen = QPen(color, width)
    pen.setCapStyle(Qt.RoundCap)
    pen.setJoinStyle(Qt.RoundJoin)
    return pen


def _limb(start: QPointF, upper: float, rel: float, l1: float, l2: float):
    a1 = math.radians(upper)
    a2 = math.radians(upper + rel)
    mid = QPointF(start.x() + l1 * math.sin(a1), start.y() + l1 * math.cos(a1))
    end = QPointF(mid.x() + l2 * math.sin(a2), mid.y() + l2 * math.cos(a2))
    return mid, end


def _leg_drop(leg: tuple[float, float]) -> float:
    upper, rel = leg
    return LEG_1 * math.cos(math.radians(upper)) + LEG_2 * math.cos(math.radians(upper + rel))


def _stroke(p: QPainter, path: QPainterPath, color: QColor, width: float) -> None:
    p.setBrush(Qt.NoBrush)
    p.setPen(_pen(OUTLINE, width + 2 * OUTLINE_W))
    p.drawPath(path)
    p.setPen(_pen(color, width))
    p.drawPath(path)


def _blob(p: QPainter, c: QPointF, rx: float, ry: float, fill: QColor) -> None:
    p.setPen(_pen(OUTLINE, OUTLINE_W))
    p.setBrush(fill)
    p.drawEllipse(c, rx, ry)


def paint_monkey(p: QPainter, pose: Pose, facing: int = 1, head_only: bool = False) -> None:
    p.save()
    p.setRenderHint(QPainter.Antialiasing, True)
    p.translate(0, pose.bob)
    p.scale(1.0 + (1.0 - pose.squash) * 0.6, pose.squash)
    p.scale(facing, 1)
    if pose.swing:
        p.translate(SCRUFF)
        p.rotate(pose.swing)
        p.translate(-SCRUFF)
    p.rotate(pose.lean)
    if pose.grounded and not head_only:
        # bending the legs lowers the hips; keep the lowest foot on the floor
        drop = max(_leg_drop(pose.leg_front), _leg_drop(pose.leg_back))
        p.translate(0, (LEG_1 + LEG_2) - drop)

    if not head_only:
        _paint_tail(p, pose.tail)
        for side, leg, color in ((-1, pose.leg_back, FUR_SHADE), (1, pose.leg_front, FUR)):
            hip = QPointF(side * HIP_X, HIP_Y)
            mid, end = _limb(hip, *leg, LEG_1, LEG_2)
            path = QPainterPath(hip)
            path.lineTo(mid)
            path.lineTo(end)
            _stroke(p, path, color, LIMB_W)
            _blob(p, QPointF(end.x() + 2.5, end.y() - 0.5), 6.5, 3.6, SKIN)
        _blob(p, QPointF(0, -44), 19, 20, FUR)
        p.setPen(Qt.NoPen)
        p.setBrush(SKIN)
        p.drawEllipse(QPointF(2, -42), 12, 15)

    p.save()
    p.translate(NECK)
    p.rotate(pose.head_tilt)
    p.translate(-NECK)
    _paint_head(p, pose, facing)
    if pose.stars is not None:
        _paint_stars(p, pose.stars)
    p.restore()

    if not head_only:
        for side, arm in ((-1, pose.arm_back), (1, pose.arm_front)):
            shoulder = QPointF(side * SHOULDER_X, SHOULDER_Y)
            mid, end = _limb(shoulder, *arm, ARM_1, ARM_2)
            path = QPainterPath(shoulder)
            path.lineTo(mid)
            path.lineTo(end)
            _stroke(p, path, FUR, LIMB_W)
            if side == 1 and pose.banana >= 0:
                _paint_banana(p, end, arm[0] + arm[1], pose.banana)
            _blob(p, end, 4.6, 4.6, SKIN)
    p.restore()

    if pose.bubble and not head_only:
        _paint_bubble(p, pose.bubble, facing, pose.bob)


def _paint_tail(p: QPainter, s: float) -> None:
    path = QPainterPath(QPointF(-12, -33))
    path.cubicTo(QPointF(-32, -30 + 4 * s), QPointF(-44 - 3 * s, -52), QPointF(-38 - 4 * s, -68))
    path.cubicTo(QPointF(-34 - 4 * s, -78), QPointF(-24 - 3 * s, -77), QPointF(-26 - 3 * s, -69))
    _stroke(p, path, FUR, 5.0)


def _paint_head(p: QPainter, pose: Pose, facing: int) -> None:
    for side in (-1, 1):
        _blob(p, QPointF(side * 22.5, -81), 9, 9, FUR)
        p.setPen(Qt.NoPen)
        p.setBrush(SKIN)
        p.drawEllipse(QPointF(side * 23, -81), 5, 5)
    _blob(p, HEAD_C, HEAD_R, HEAD_R, FUR)

    fx = FACE_DX
    face = QPainterPath()
    face.setFillRule(Qt.WindingFill)  # union of the overlapping ellipses
    face.addEllipse(QPointF(fx - 7.5, -85), 9.5, 9.5)
    face.addEllipse(QPointF(fx + 7.5, -85), 9.5, 9.5)
    face.addEllipse(QPointF(fx, -71), 14, 10)
    p.setPen(_pen(OUTLINE, 1.1))
    p.setBrush(SKIN)
    p.drawPath(face.simplified())

    lx, ly = pose.look
    lx *= facing  # painter is mirrored; look is given in screen space
    for ex in (fx - 7.5, fx + 7.5):
        c = QPointF(ex, -86)
        if pose.eyes in ("open", "wide"):
            wide = pose.eyes == "wide"
            _blob(p, c, 5.2 if wide else 4.4, 5.2 if wide else 4.4, WHITE)
            r, k = (2.2, 1.0) if wide else (2.7, 1.5)
            pupil = QPointF(c.x() + lx * k, c.y() + ly * k)
            p.setPen(Qt.NoPen)
            p.setBrush(OUTLINE)
            p.drawEllipse(pupil, r, r)
            p.setBrush(WHITE)
            p.drawEllipse(QPointF(pupil.x() - 0.9, pupil.y() - 0.9), 0.9, 0.9)
        else:
            path = QPainterPath()
            if pose.eyes == "closed":
                path.moveTo(c.x() - 3.6, c.y())
                path.quadTo(c.x(), c.y() + 2.6, c.x() + 3.6, c.y())
            elif pose.eyes == "happy":
                path.moveTo(c.x() - 3.6, c.y() + 1.2)
                path.quadTo(c.x(), c.y() - 3.4, c.x() + 3.6, c.y() + 1.2)
            else:  # dizzy
                path.moveTo(c.x() - 3, c.y() - 3)
                path.lineTo(c.x() + 3, c.y() + 3)
                path.moveTo(c.x() + 3, c.y() - 3)
                path.lineTo(c.x() - 3, c.y() + 3)
            p.setBrush(Qt.NoBrush)
            p.setPen(_pen(OUTLINE, 1.7))
            p.drawPath(path)

    p.setPen(Qt.NoPen)
    p.setBrush(OUTLINE)
    for nx in (fx - 2.3, fx + 2.3):
        p.drawEllipse(QPointF(nx, -75), 1.1, 1.1)

    y = -69.5
    mouth = QPainterPath()
    if pose.mouth == "big":
        mouth.moveTo(fx - 6.5, y)
        mouth.quadTo(fx, y + 9, fx + 6.5, y)
        mouth.closeSubpath()
        p.setPen(_pen(OUTLINE, 1.3))
        p.setBrush(MOUTH)
        p.drawPath(mouth)
    elif pose.mouth == "o":
        p.setPen(_pen(OUTLINE, 1.1))
        p.setBrush(MOUTH)
        p.drawEllipse(QPointF(fx, -67), 2.8, 3.3)
    else:
        if pose.mouth == "chew":
            mouth.moveTo(fx - 4, y + 1.5)
            mouth.lineTo(fx + 4, y + 1.5)
        else:
            mouth.moveTo(fx - 6, y)
            mouth.quadTo(fx, y + 4.5, fx + 6, y)
        p.setBrush(Qt.NoBrush)
        p.setPen(_pen(OUTLINE, 1.6))
        p.drawPath(mouth)


def _paint_banana(p: QPainter, hand: QPointF, forearm_deg: float, bites: int) -> None:
    p.save()
    p.translate(hand)
    p.rotate((180.0 - forearm_deg) * 0.7)  # point roughly along the forearm
    length = 28.0 * (1.0 - 0.25 * bites)
    if bites:  # peel flaps hanging down
        p.setPen(_pen(BANANA_EDGE, 1.0))
        p.setBrush(BANANA)
        p.drawPolygon(QPolygonF([QPointF(-4, -3), QPointF(-9, 5), QPointF(-5, 4)]))
        p.drawPolygon(QPolygonF([QPointF(1, -3), QPointF(7, 6), QPointF(3, 4)]))
    body = QPainterPath(QPointF(2.5, 2))
    body.cubicTo(QPointF(-1, -length * 0.3), QPointF(-1, -length * 0.75), QPointF(-4, -length))
    body.cubicTo(QPointF(-9, -length * 0.7), QPointF(-8, -length * 0.25), QPointF(-2.5, 2))
    body.closeSubpath()
    p.setPen(_pen(BANANA_EDGE, 1.2))
    p.setBrush(BANANA if not bites else BANANA_FLESH)
    p.drawPath(body)
    if not bites:
        p.setPen(Qt.NoPen)
        p.setBrush(OUTLINE)
        p.drawEllipse(QPointF(-4, -length), 1.2, 1.2)
    p.setPen(Qt.NoPen)
    p.setBrush(OUTLINE)
    p.drawRect(QRectF(-1.2, 1.5, 2.6, 3.5))  # stem
    p.restore()


def _paint_stars(p: QPainter, angle: float) -> None:
    for i in range(3):
        a = angle + i * 2 * math.pi / 3
        c = QPointF(FACE_DX + 25 * math.cos(a), -104 + 5 * math.sin(a))
        pts = []
        for k in range(10):
            r = 4.6 if k % 2 == 0 else 2.0
            t = -math.pi / 2 + k * math.pi / 5 + a * 2
            pts.append(QPointF(c.x() + r * math.cos(t), c.y() + r * math.sin(t)))
        p.setPen(_pen(BANANA_EDGE, 1.0))
        p.setBrush(STAR)
        p.drawPolygon(QPolygonF(pts))


def _paint_bubble(p: QPainter, text: str, facing: int, bob: float) -> None:
    p.save()
    p.setRenderHint(QPainter.Antialiasing, True)
    font = QFont()
    font.setBold(True)
    font.setPixelSize(11)
    p.setFont(font)
    w = QFontMetricsF(font).horizontalAdvance(text) + 14
    h = 20.0
    ax, ay = facing * 14.0, -112.0 + bob          # tip of the bubble's tail
    x = ax - 8 if facing > 0 else ax + 8 - w
    x = max(-82.0, min(82.0 - w, x))
    rect = QRectF(x, ay - 8 - h, w, h)
    shape = QPainterPath()
    shape.addRoundedRect(rect, 8, 8)
    tail = QPainterPath(QPointF(ax - 4, rect.bottom() - 1))
    tail.lineTo(ax, ay)
    tail.lineTo(ax + 4, rect.bottom() - 1)
    tail.closeSubpath()
    p.setPen(_pen(OUTLINE, 1.4))
    p.setBrush(WHITE)
    p.drawPath(shape.united(tail))
    p.setPen(OUTLINE)
    p.drawText(rect, Qt.AlignCenter, text)
    p.restore()


def render_icon(size: int = 64) -> QPixmap:
    """The monkey's face, for the tray / window icon."""
    pm = QPixmap(size, size)
    pm.fill(Qt.transparent)
    p = QPainter(pm)
    p.translate(size / 2, size / 2)
    p.scale(size / 64, size / 64)
    p.translate(-FACE_DX / 2, 80)
    paint_monkey(p, Pose(), 1, head_only=True)
    p.end()
    return pm

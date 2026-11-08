"""Procedural crow renderer (side view): every frame is painted from a CrowPose.

Coordinate system (before the caller's scale): origin between the feet,
y grows downward, the crow faces +x. The painter is mirrored for facing=-1.

Body, neck, head and tail are unioned into one glossy silhouette;
the wing, beak, eye and any coin are layered on top.
"""
from __future__ import annotations

import math

from PySide6.QtCore import QPointF, Qt
from PySide6.QtGui import QColor, QPainter, QPainterPath, QPolygonF, QTransform

from ...art import paint_bubble, paint_stars, pen, polyline, tapered_curve
from .brain import CrowPose

BODY = QColor("#24252E")
GLOSS = QColor(110, 125, 190, 70)       # blue-violet sheen on black feathers
WING = QColor("#2D2F3B")
FEATHER_LINE = QColor("#565C75")
OUTLINE = QColor("#0C0C10")
BEAK = QColor("#2A2A31")
BEAK_RIDGE = QColor("#575864")
LEGS = QColor("#3A3A41")
EYE_RING = QColor("#F2F2F2")
PUPIL = QColor("#140E0B")
WHITE = QColor("#FFFFFF")
GOLD = QColor("#F2C230")
GOLD_LIGHT = QColor("#FFE47A")
GOLD_EDGE = QColor("#9A7410")

LINE_W = 1.6
HEAD_R = 11.0
NECK_BASE = QPointF(8, -44)
PIVOT = QPointF(0, -46)          # grab point when dangling from the cursor
HIP_BACK, HIP_FRONT = QPointF(-6, -21), QPointF(2, -21)
LEG = 17.0
TAIL_ROOT = QPointF(-19, -31)
TAIL_DROOP = 12.0               # degrees the resting tail points below horizontal

# Extents used by the window for layout / screen-edge collisions
HALF_WIDTH = 50.0
HEIGHT = 70.0
EYE_Y = 57.0


def paint(p: QPainter, pose: CrowPose, facing: int = 1, head_only: bool = False) -> None:
    p.save()
    p.setRenderHint(QPainter.Antialiasing, True)
    p.translate(0, pose.bob)
    p.scale(1.0 + (1.0 - pose.squash) * 0.6, pose.squash)
    p.scale(facing, 1)
    if pose.coin == "ground" and not head_only:
        _paint_coin(p, QPointF(33, -1.6), 4.2, 1.7, 0.0)   # on the floor, not leaning with the bird
    if pose.swing:
        p.translate(PIVOT)
        p.rotate(pose.swing)
        p.translate(-PIVOT)
    p.rotate(pose.lean)
    if pose.grounded and not head_only:
        drop = max(math.cos(math.radians(pose.leg_front)), math.cos(math.radians(pose.leg_back))) * LEG
        p.translate(0, LEG - drop)

    head = QPointF(*pose.head)
    skull = QPainterPath()
    skull.addEllipse(head, HEAD_R, HEAD_R)
    sil = _neck_path(head).united(skull)
    if not head_only:
        sil = sil.united(_body_path()).united(_tail_path(pose.tail))
        for hip, angle in ((HIP_BACK, pose.leg_back), (HIP_FRONT, pose.leg_front)):
            _paint_leg(p, hip, angle)

    p.setPen(Qt.NoPen)
    p.setBrush(BODY)
    p.drawPath(sil)
    p.save()                             # sheen along the back and crown
    p.setClipPath(sil)
    p.setBrush(GLOSS)
    if not head_only:
        p.drawEllipse(QPointF(-6, -46), 20, 5)
    p.drawEllipse(QPointF(head.x() - 2, head.y() - 7), 7, 3)
    p.restore()
    p.setPen(pen(OUTLINE, LINE_W))
    p.setBrush(Qt.NoBrush)
    p.drawPath(sil)

    if not head_only:
        _paint_wing(p, pose.wing)
    _paint_head(p, pose, head, facing)
    if pose.stars is not None:
        paint_stars(p, head.x(), head.y() - 17, pose.stars, rx=17, ry=4)
    p.restore()

    if pose.bubble and not head_only:
        paint_bubble(p, pose.bubble, facing * pose.head[0], pose.head[1] - 14 + pose.bob, facing,
                     outline=OUTLINE)


def _neck_path(head: QPointF) -> QPainterPath:
    p0 = (NECK_BASE.x(), NECK_BASE.y())
    return tapered_curve(p0, (p0[0] + 3, p0[1] - 5), (head.x() - 3, head.y() + 6),
                         (head.x(), head.y()), 19.0, 15.0)


def _body_path() -> QPainterPath:
    body = QPainterPath()
    body.addEllipse(QPointF(0, 0), 22, 13.5)
    return QTransform().translate(-2, -33).rotate(-18).map(body)   # chest held high


def _tail_path(angle: float) -> QPainterPath:
    fan = QPainterPath()
    fan.addPolygon(QPolygonF([QPointF(2, -4), QPointF(-23, -6), QPointF(-27, -1), QPointF(-28, 4),
                              QPointF(-25, 10), QPointF(2, 4)]))   # narrow root, wide end
    fan.closeSubpath()
    return QTransform().translate(TAIL_ROOT.x(), TAIL_ROOT.y()).rotate(TAIL_DROOP - angle).map(fan)


def _paint_leg(p: QPainter, hip: QPointF, angle: float) -> None:
    a = math.radians(angle)
    x, y = hip.x() + LEG * math.sin(a), hip.y() + LEG * math.cos(a)
    p.setBrush(Qt.NoBrush)
    p.setPen(pen(LEGS, 2.4))
    p.drawLine(hip, QPointF(x, y))
    p.setPen(pen(LEGS, 1.8))
    p.drawPath(polyline((x - 4.5, y + 2.6), (x, y + 2.2), (x + 7.5, y + 2.6)))  # back + front toes
    p.drawLine(QPointF(x, y + 2.2), QPointF(x + 5, y + 3.2))


def _paint_wing(p: QPainter, raised: float) -> None:
    p.save()
    p.translate(6, -43)          # shoulder
    p.rotate(105 * raised)       # folded along the back -> raised (negative: down-stroke)
    wing = QPainterPath(QPointF(3, -3))
    wing.cubicTo(QPointF(-10, -8), QPointF(-30, -4), QPointF(-42, 6))   # leading edge to tip
    wing.quadTo(QPointF(-36, 7), QPointF(-33, 10))                      # long primaries...
    wing.quadTo(QPointF(-29, 8), QPointF(-25, 12))
    wing.quadTo(QPointF(-20, 9), QPointF(-15, 12))
    wing.cubicTo(QPointF(-6, 11), QPointF(2, 5), QPointF(3, -3))
    p.setPen(Qt.NoPen)
    p.setBrush(WING)
    p.drawPath(wing)
    p.setPen(pen(FEATHER_LINE, 1.0))
    for (x1, y1), (x2, y2) in (((-12, 5), (-30, 4)), ((-9, 0), (-34, 2)), ((-16, 9), (-26, 9))):
        p.drawLine(QPointF(x1, y1), QPointF(x2, y2))
    p.setPen(pen(OUTLINE, LINE_W))
    p.setBrush(Qt.NoBrush)
    p.drawPath(wing)
    p.restore()


def _paint_coin(p: QPainter, c: QPointF, rx: float, ry: float, glint: float) -> None:
    p.setPen(pen(GOLD_EDGE, 1.0))
    p.setBrush(GOLD)
    p.drawEllipse(c, rx, ry)
    p.setPen(pen(GOLD_LIGHT, 0.8))
    p.setBrush(Qt.NoBrush)
    p.drawEllipse(c, rx * 0.55, ry * 0.55)
    if glint > 0.05:             # four-point sparkle
        s = 2 + 6 * glint
        g = QPointF(c.x() + rx * 0.4, c.y() - ry * 0.7)
        sparkle = QPolygonF([QPointF(g.x(), g.y() - s), QPointF(g.x() + s * 0.22, g.y() - s * 0.22),
                             QPointF(g.x() + s, g.y()), QPointF(g.x() + s * 0.22, g.y() + s * 0.22),
                             QPointF(g.x(), g.y() + s), QPointF(g.x() - s * 0.22, g.y() + s * 0.22),
                             QPointF(g.x() - s, g.y()), QPointF(g.x() - s * 0.22, g.y() - s * 0.22)])
        p.setPen(Qt.NoPen)
        p.setBrush(WHITE)
        p.drawPolygon(sparkle)


def _paint_head(p: QPainter, pose: CrowPose, head: QPointF, facing: int) -> None:
    p.save()
    p.translate(head)
    p.rotate(pose.head_angle)

    p.save()                     # lower mandible hinges open
    p.translate(7, 2)
    p.rotate(28 * min(1.0, pose.beak_open))
    lower = polyline((0, -0.4), (14, 0.6), (13.5, 1.8), (0, 3.4))
    lower.closeSubpath()
    p.setPen(pen(OUTLINE, 1.1))
    p.setBrush(BEAK)
    p.drawPath(lower)
    p.restore()

    upper = QPainterPath(QPointF(6, -5))
    upper.cubicTo(QPointF(13, -5.5), QPointF(19, -3), QPointF(23, 1))   # heavy curved culmen
    upper.quadTo(QPointF(21, 2.2), QPointF(18, 2.0))
    upper.lineTo(QPointF(7, 2.4))
    upper.quadTo(QPointF(4.5, -1), QPointF(6, -5))
    upper.closeSubpath()
    p.setPen(pen(OUTLINE, 1.1))
    p.setBrush(BEAK)
    p.drawPath(upper)
    p.setPen(pen(BEAK_RIDGE, 0.9))
    p.drawLine(QPointF(9, -4), QPointF(19, -1.6))
    if pose.coin == "beak":
        _paint_coin(p, QPointF(20, 3.4), 3.8, 3.2, pose.glint)

    eye = QPointF(1.5, -3)
    lx, ly = pose.look
    lx *= facing
    if pose.eyes in ("open", "wide"):
        wide = pose.eyes == "wide"
        p.setPen(Qt.NoPen)
        p.setBrush(EYE_RING)
        ring = 3.8 if wide else 3.0
        p.drawEllipse(eye, ring, ring)
        r, k = (1.8, 1.0) if wide else (2.1, 0.7)
        pupil = QPointF(eye.x() + lx * k, eye.y() + ly * k)
        p.setBrush(PUPIL)
        p.drawEllipse(pupil, r, r)
        p.setBrush(WHITE)
        p.drawEllipse(QPointF(pupil.x() - 0.6, pupil.y() - 0.7), 0.7, 0.7)
    else:
        path = QPainterPath()
        x, y = eye.x(), eye.y()
        if pose.eyes == "closed":
            path.moveTo(x - 2.8, y)
            path.quadTo(x, y + 2, x + 2.8, y)
        elif pose.eyes == "happy":
            path.moveTo(x - 2.8, y + 1)
            path.quadTo(x, y - 2.6, x + 2.8, y + 1)
        else:  # dizzy
            path.moveTo(x - 2.3, y - 2.3)
            path.lineTo(x + 2.3, y + 2.3)
            path.moveTo(x + 2.3, y - 2.3)
            path.lineTo(x - 2.3, y + 2.3)
        p.setBrush(Qt.NoBrush)
        p.setPen(pen(EYE_RING, 1.6))     # light strokes so they read on black feathers
        p.drawPath(path)
    p.restore()

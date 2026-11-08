"""Green parrot painter: yellow forehead, red shoulder, blue flight feathers, hooked beak."""
from __future__ import annotations

import math

from PySide6.QtCore import QPointF, Qt
from PySide6.QtGui import QColor, QPainter, QPainterPath, QPolygonF, QTransform

from ...art import paint_bubble, paint_stars, paint_zzz, pen, polyline, tapered_curve
from .brain import ParrotPose

GREEN = QColor("#45AE49")
GREEN_LIGHT = QColor("#79CC5E")
GREEN_DARK = QColor("#2C8B3A")
YELLOW = QColor("#F4D23C")
RED = QColor("#D8352A")
BLUE = QColor("#2E66C6")
TAIL_TIP = QColor("#BFD94A")
BEAK = QColor("#DCCDA8")
BEAK_DARK = QColor("#8C7E62")
EYE_RING = QColor("#F2F1EC")
IRIS = QColor("#F39A1E")
PUPIL = QColor("#141010")
WHITE = QColor("#FFFFFF")
LEGS = QColor("#8C8C93")
OUTLINE = QColor("#173A1C")

LINE_W = 1.6
HEAD_R = 11.5
NECK_BASE = QPointF(3, -44)
PIVOT = QPointF(0, -50)
HIP_BACK, HIP_FRONT = QPointF(-3, -14), QPointF(3, -14)
LEG = 11.0
TAIL_ROOT = QPointF(-7, -19)

HALF_WIDTH = 42.0
HEIGHT = 72.0
EYE_Y = 59.0


def paint(p: QPainter, pose: ParrotPose, facing: int = 1, head_only: bool = False) -> None:
    p.save()
    p.setRenderHint(QPainter.Antialiasing, True)
    p.translate(0, pose.bob)
    p.scale(1.0 + (1.0 - pose.squash) * 0.6, pose.squash)
    p.scale(facing, 1)
    if pose.swing:
        p.translate(PIVOT)
        p.rotate(pose.swing)
        p.translate(-PIVOT)
    p.rotate(pose.lean)

    head = QPointF(*pose.head)
    puff = 1.0 + 0.12 * pose.fluff
    skull = QPainterPath()
    skull.addEllipse(head, HEAD_R * puff, HEAD_R * puff)
    neck = tapered_curve((NECK_BASE.x(), NECK_BASE.y()), (NECK_BASE.x() + 1, NECK_BASE.y() - 4),
                         (head.x() - 2, head.y() + 5), (head.x(), head.y()), 21.0 * puff, 19.0 * puff)
    sil = neck.united(skull)
    body = None
    if not head_only:
        _paint_tail(p, pose.tail)
        for hip, angle in ((HIP_BACK, pose.leg_back), (HIP_FRONT, pose.leg_front)):
            _paint_leg(p, hip, angle)
        body = QPainterPath()
        body.addEllipse(QPointF(0, 0), 13.5 * puff, 19.0 * puff)
        body = QTransform().translate(0, -33).rotate(16).map(body)    # upright, chest forward
        sil = sil.united(body)

    p.setPen(Qt.NoPen)
    p.setBrush(GREEN)
    p.drawPath(sil)
    p.save()
    p.setClipPath(sil)
    if body is not None:
        p.setBrush(GREEN_LIGHT)
        p.drawEllipse(QPointF(7, -28), 9, 14)                      # lighter breast
    p.setBrush(YELLOW)
    p.drawEllipse(QPointF(head.x() + 3, head.y() - 9), 8, 5.5)     # yellow forehead
    p.restore()
    p.setPen(pen(OUTLINE, LINE_W))
    p.setBrush(Qt.NoBrush)
    p.drawPath(sil)

    if not head_only:
        _paint_wing(p, pose.wing)
    _paint_head(p, pose, head, facing)
    if pose.stars is not None:
        paint_stars(p, head.x(), head.y() - 17, pose.stars, rx=16, ry=4)
    if pose.notes is not None and not head_only:
        paint_zzz(p, head.x() + 10, head.y() - 6, pose.notes, facing, glyphs="♪♫♪", color=BLUE)
    p.restore()

    if pose.bubble and not head_only:
        paint_bubble(p, pose.bubble, facing * pose.head[0], pose.head[1] - 15 + pose.bob, facing,
                     outline=OUTLINE)


def _paint_tail(p: QPainter, angle: float) -> None:
    p.save()
    p.translate(TAIL_ROOT)
    p.rotate(-angle)
    tail = tapered_curve((0, 0), (-7, 5), (-15, 10), (-23, 15), 11.0, 7.0, steps=10, power=1.0)
    p.setPen(Qt.NoPen)
    p.setBrush(GREEN_DARK)
    p.drawPath(tail)
    p.save()
    p.setClipPath(tail)
    p.setBrush(TAIL_TIP)
    p.drawEllipse(QPointF(-23, 15), 7, 7)
    p.restore()
    p.setPen(pen(OUTLINE, LINE_W))
    p.setBrush(Qt.NoBrush)
    p.drawPath(tail)
    p.restore()


def _paint_leg(p: QPainter, hip: QPointF, angle: float) -> None:
    a = math.radians(angle)
    x, y = hip.x() + LEG * math.sin(a), hip.y() + LEG * math.cos(a)
    p.setBrush(Qt.NoBrush)
    p.setPen(pen(LEGS, 3.0))
    p.drawLine(hip, QPointF(x, y))
    p.setPen(pen(LEGS, 2.0))             # zygodactyl: two toes forward, two back
    p.drawPath(polyline((x + 5.5, y + 2.4), (x, y + 1.6), (x - 4.5, y + 2.6)))
    p.drawLine(QPointF(x, y + 1.6), QPointF(x + 4, y + 3.2))


def _paint_wing(p: QPainter, raised: float) -> None:
    p.save()
    p.translate(1, -44)          # shoulder
    p.rotate(105 * raised)
    wing = QPainterPath(QPointF(4, -2))
    wing.cubicTo(QPointF(-4, -6), QPointF(-12, 4), QPointF(-14, 26))    # down the back to the tip
    wing.quadTo(QPointF(-10, 24), QPointF(-8, 27))
    wing.quadTo(QPointF(-5, 22), QPointF(-2, 22))
    wing.cubicTo(QPointF(4, 14), QPointF(7, 4), QPointF(4, -2))
    p.setPen(Qt.NoPen)
    p.setBrush(GREEN_DARK)
    p.drawPath(wing)
    p.save()
    p.setClipPath(wing)
    p.setBrush(BLUE)
    p.drawEllipse(QPointF(-9, 26), 8, 8)       # blue flight feathers
    p.setBrush(RED)
    p.drawEllipse(QPointF(1, 1), 5, 4.5)       # red shoulder patch
    p.restore()
    p.setPen(pen(GREEN_LIGHT, 0.9))
    p.drawLine(QPointF(-3, 10), QPointF(-9, 20))
    p.drawLine(QPointF(1, 11), QPointF(-4, 20))
    p.setPen(pen(OUTLINE, LINE_W))
    p.setBrush(Qt.NoBrush)
    p.drawPath(wing)
    p.restore()


def _paint_head(p: QPainter, pose: ParrotPose, head: QPointF, facing: int) -> None:
    p.save()
    p.translate(head)
    p.rotate(pose.head_angle)

    p.save()                     # lower mandible hinges open
    p.translate(7.5, 3)
    p.rotate(25 * min(1.0, pose.beak_open))
    lower = polyline((0, -0.5), (6.5, 0.2), (5.5, 3.2), (0, 4))
    lower.closeSubpath()
    p.setPen(pen(OUTLINE, 1.1))
    p.setBrush(BEAK_DARK)
    p.drawPath(lower)
    p.restore()

    upper = QPainterPath(QPointF(6, -6.5))
    upper.cubicTo(QPointF(13, -8.5), QPointF(19, -3), QPointF(16.5, 7))     # big hooked upper beak
    upper.quadTo(QPointF(15, 3.5), QPointF(12, 2.6))
    upper.lineTo(QPointF(7, 3.2))
    upper.quadTo(QPointF(4.5, -1.5), QPointF(6, -6.5))
    upper.closeSubpath()
    p.setPen(pen(OUTLINE, 1.1))
    p.setBrush(BEAK)
    p.drawPath(upper)
    p.setPen(pen(BEAK_DARK, 0.9))
    p.drawLine(QPointF(8, -5), QPointF(14, -3))

    eye = QPointF(1.0, -3.0)
    lx, ly = pose.look
    lx *= facing
    p.setPen(Qt.NoPen)
    p.setBrush(EYE_RING)                         # bare white eye ring
    p.drawEllipse(eye, 4.6, 4.6)
    if pose.eyes in ("open", "wide"):
        r = 2.9 if pose.eyes == "wide" else 2.6
        c = QPointF(eye.x() + lx * 0.6, eye.y() + ly * 0.6)
        p.setBrush(IRIS)
        p.drawEllipse(c, r, r)
        p.setBrush(PUPIL)
        p.drawEllipse(c, r * 0.5, r * 0.5)
        p.setBrush(WHITE)
        p.drawEllipse(QPointF(c.x() - 0.8, c.y() - 0.9), 0.7, 0.7)
    else:
        path = QPainterPath()
        x, y = eye.x(), eye.y()
        if pose.eyes == "closed":
            path.moveTo(x - 3, y)
            path.quadTo(x, y + 2.2, x + 3, y)
        elif pose.eyes == "happy":
            path.moveTo(x - 3, y + 1)
            path.quadTo(x, y - 2.8, x + 3, y + 1)
        else:  # dizzy
            path.moveTo(x - 2.4, y - 2.4)
            path.lineTo(x + 2.4, y + 2.4)
            path.moveTo(x + 2.4, y - 2.4)
            path.lineTo(x - 2.4, y + 2.4)
        p.setBrush(Qt.NoBrush)
        p.setPen(pen(PUPIL, 1.5))
        p.drawPath(path)
    p.setPen(pen(OUTLINE, 0.9))
    p.setBrush(Qt.NoBrush)
    p.drawEllipse(eye, 4.6, 4.6)
    p.restore()

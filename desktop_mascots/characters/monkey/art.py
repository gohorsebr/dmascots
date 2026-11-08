"""Procedural monkey renderer: every frame is painted from a MonkeyPose.

Coordinate system (before the caller's scale): origin between the feet,
y grows downward, the monkey faces +x. The painter is mirrored for facing=-1.
"""
from __future__ import annotations

import math

from PySide6.QtCore import QPointF, QRectF, Qt
from PySide6.QtGui import QColor, QPainter, QPainterPath, QPolygonF

from ...art import OUTLINE, WHITE, blob, paint_bubble, paint_stars, pen, polyline, stroke
from .brain import MonkeyPose

FUR = QColor("#8B5A2B")
FUR_SHADE = QColor("#72461F")
SKIN = QColor("#EECDA3")
MOUTH = QColor("#7A2424")
BANANA = QColor("#F6CE3A")
BANANA_EDGE = QColor("#A9800F")
BANANA_FLESH = QColor("#FFF4C9")

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
HALF_WIDTH = 46.0
HEIGHT = 112.0
EYE_Y = 84.0


def _limb(start: QPointF, upper: float, rel: float, l1: float, l2: float):
    a1 = math.radians(upper)
    a2 = math.radians(upper + rel)
    mid = QPointF(start.x() + l1 * math.sin(a1), start.y() + l1 * math.cos(a1))
    end = QPointF(mid.x() + l2 * math.sin(a2), mid.y() + l2 * math.cos(a2))
    return mid, end


def _leg_drop(leg: tuple[float, float]) -> float:
    upper, rel = leg
    return LEG_1 * math.cos(math.radians(upper)) + LEG_2 * math.cos(math.radians(upper + rel))


def _limb_path(start: QPointF, mid: QPointF, end: QPointF) -> QPainterPath:
    path = QPainterPath(start)
    path.lineTo(mid)
    path.lineTo(end)
    return path


def paint(p: QPainter, pose: MonkeyPose, facing: int = 1, head_only: bool = False) -> None:
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
            stroke(p, _limb_path(hip, mid, end), color, LIMB_W)
            blob(p, QPointF(end.x() + 2.5, end.y() - 0.5), 6.5, 3.6, SKIN)
        blob(p, QPointF(0, -44), 19, 20, FUR)
        p.setPen(Qt.NoPen)
        p.setBrush(SKIN)
        p.drawEllipse(QPointF(2, -42), 12, 15)

    p.save()
    p.translate(NECK)
    p.rotate(pose.head_tilt)
    p.translate(-NECK)
    _paint_head(p, pose, facing)
    if pose.stars is not None:
        paint_stars(p, FACE_DX, -104, pose.stars)
    p.restore()

    if not head_only:
        for side, arm in ((-1, pose.arm_back), (1, pose.arm_front)):
            shoulder = QPointF(side * SHOULDER_X, SHOULDER_Y)
            mid, end = _limb(shoulder, *arm, ARM_1, ARM_2)
            stroke(p, _limb_path(shoulder, mid, end), FUR, LIMB_W)
            if side == 1 and pose.banana >= 0:
                _paint_banana(p, end, arm[0] + arm[1], pose.banana)
            blob(p, end, 4.6, 4.6, SKIN)
    p.restore()

    if pose.bubble and not head_only:
        paint_bubble(p, pose.bubble, facing * 14.0, -112.0 + pose.bob, facing)


def _paint_tail(p: QPainter, s: float) -> None:
    path = QPainterPath(QPointF(-12, -33))
    path.cubicTo(QPointF(-32, -30 + 4 * s), QPointF(-44 - 3 * s, -52), QPointF(-38 - 4 * s, -68))
    path.cubicTo(QPointF(-34 - 4 * s, -78), QPointF(-24 - 3 * s, -77), QPointF(-26 - 3 * s, -69))
    stroke(p, path, FUR, 5.0)


def _paint_head(p: QPainter, pose: MonkeyPose, facing: int) -> None:
    for side in (-1, 1):
        blob(p, QPointF(side * 22.5, -81), 9, 9, FUR)
        p.setPen(Qt.NoPen)
        p.setBrush(SKIN)
        p.drawEllipse(QPointF(side * 23, -81), 5, 5)
    blob(p, HEAD_C, HEAD_R, HEAD_R, FUR)

    fx = FACE_DX
    face = QPainterPath()
    face.setFillRule(Qt.WindingFill)  # union of the overlapping ellipses
    face.addEllipse(QPointF(fx - 7.5, -85), 9.5, 9.5)
    face.addEllipse(QPointF(fx + 7.5, -85), 9.5, 9.5)
    face.addEllipse(QPointF(fx, -71), 14, 10)
    p.setPen(pen(OUTLINE, 1.1))
    p.setBrush(SKIN)
    p.drawPath(face.simplified())

    lx, ly = pose.look
    lx *= facing  # painter is mirrored; look is given in screen space
    for ex in (fx - 7.5, fx + 7.5):
        c = QPointF(ex, -86)
        if pose.eyes in ("open", "wide"):
            wide = pose.eyes == "wide"
            blob(p, c, 5.2 if wide else 4.4, 5.2 if wide else 4.4, WHITE)
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
            p.setPen(pen(OUTLINE, 1.7))
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
        p.setPen(pen(OUTLINE, 1.3))
        p.setBrush(MOUTH)
        p.drawPath(mouth)
    elif pose.mouth == "o":
        p.setPen(pen(OUTLINE, 1.1))
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
        p.setPen(pen(OUTLINE, 1.6))
        p.drawPath(mouth)


def _paint_banana(p: QPainter, hand: QPointF, forearm_deg: float, bites: int) -> None:
    p.save()
    p.translate(hand)
    p.rotate((180.0 - forearm_deg) * 0.7)  # point roughly along the forearm
    length = 28.0 * (1.0 - 0.25 * bites)
    if bites:  # peel flaps hanging down
        p.setPen(pen(BANANA_EDGE, 1.0))
        p.setBrush(BANANA)
        p.drawPolygon(QPolygonF([QPointF(-4, -3), QPointF(-9, 5), QPointF(-5, 4)]))
        p.drawPolygon(QPolygonF([QPointF(1, -3), QPointF(7, 6), QPointF(3, 4)]))
    body = QPainterPath(QPointF(2.5, 2))
    body.cubicTo(QPointF(-1, -length * 0.3), QPointF(-1, -length * 0.75), QPointF(-4, -length))
    body.cubicTo(QPointF(-9, -length * 0.7), QPointF(-8, -length * 0.25), QPointF(-2.5, 2))
    body.closeSubpath()
    p.setPen(pen(BANANA_EDGE, 1.2))
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

"""Procedural goose renderer (side view): every frame is painted from a GoosePose.

Coordinate system (before the caller's scale): origin between the feet,
y grows downward, the goose faces +x. The painter is mirrored for facing=-1.
"""
from __future__ import annotations

import math

from PySide6.QtCore import QPointF, Qt
from PySide6.QtGui import QColor, QPainter, QPainterPath, QPolygonF

from ...art import paint_bubble, paint_stars, pen, polyline, stroke
from .brain import GoosePose

FEATHER = QColor("#FBFBF7")
WING = QColor("#ECECE5")
WING_LINE = QColor("#B4B4AA")
OUTLINE = QColor("#2E2E2E")
ORANGE = QColor("#F39A2B")
ORANGE_DARK = QColor("#C9711B")
EYE = QColor("#151515")
WHITE = QColor("#FFFFFF")

LINE_W = 1.6
NECK_W = 10.0
HEAD_R = 9.5
NECK_BASE = QPointF(13, -44)
PIVOT = QPointF(0, -52)         # grab point when dangling from the cursor
HIP_BACK, HIP_FRONT = QPointF(-5, -21), QPointF(3, -21)
LEG = 19.0

# Extents used by the window for layout / screen-edge collisions
HALF_WIDTH = 50.0
HEIGHT = 92.0
EYE_Y = 81.0


def _foot(angle: float, hip: QPointF) -> QPointF:
    a = math.radians(angle)
    return QPointF(hip.x() + LEG * math.sin(a), hip.y() + LEG * math.cos(a))


def paint(p: QPainter, pose: GoosePose, facing: int = 1, head_only: bool = False) -> None:
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
    if pose.grounded and not head_only:
        drop = max(math.cos(math.radians(pose.leg_front)), math.cos(math.radians(pose.leg_back))) * LEG
        p.translate(0, LEG - drop)

    head = QPointF(*pose.head)
    neck = _neck_path(head, pose.neck_bend)
    body = None if head_only else _body_path(pose.tail)

    if not head_only:
        for hip, angle, color in ((HIP_BACK, pose.leg_back, ORANGE_DARK), (HIP_FRONT, pose.leg_front, ORANGE)):
            foot = _foot(angle, hip)
            stroke(p, polyline((hip.x(), hip.y()), (foot.x(), foot.y())), color, 3.0,
                   outline=OUTLINE, outline_w=1.1)
            _paint_foot(p, foot, color)

    # Neck, body and head share one silhouette: all the outlines first, then all the fills.
    p.setBrush(Qt.NoBrush)
    p.setPen(pen(OUTLINE, NECK_W + 2 * LINE_W))
    p.drawPath(neck)
    if body is not None:
        p.setPen(pen(OUTLINE, 2 * LINE_W))
        p.setBrush(OUTLINE)
        p.drawPath(body)
    p.setPen(Qt.NoPen)
    p.setBrush(OUTLINE)
    p.drawEllipse(head, HEAD_R + LINE_W, HEAD_R + LINE_W)

    p.setBrush(Qt.NoBrush)
    p.setPen(pen(FEATHER, NECK_W))
    p.drawPath(neck)
    p.setPen(Qt.NoPen)
    p.setBrush(FEATHER)
    if body is not None:
        p.drawPath(body)
    p.drawEllipse(head, HEAD_R, HEAD_R)

    if not head_only:
        _paint_wing(p, pose.wing)
    _paint_head(p, pose, head, facing)
    if pose.stars is not None:
        paint_stars(p, head.x(), head.y() - 16, pose.stars, rx=17, ry=4)
    p.restore()

    if pose.bubble and not head_only:
        paint_bubble(p, pose.bubble, facing * pose.head[0], pose.head[1] - 13 + pose.bob, facing,
                     outline=OUTLINE)


def _neck_path(head: QPointF, bend: float) -> QPainterPath:
    b = NECK_BASE
    path = QPainterPath(b)
    path.cubicTo(QPointF(b.x() + 6 + bend, b.y() - 12),
                 QPointF(head.x() - 4 + bend * 0.5, head.y() + 14), head)
    return path


def _body_path(tail: float) -> QPainterPath:
    body = QPainterPath()
    body.addEllipse(QPointF(-4, -36), 27, 17)
    tip = polyline((-22, -46), (-42, -52 + 3 * tail), (-26, -31))
    tip.closeSubpath()
    return body.united(tip)


def _paint_foot(p: QPainter, ankle: QPointF, color: QColor) -> None:
    x, y = ankle.x(), ankle.y()
    web = QPolygonF([QPointF(x - 2, y - 1.5), QPointF(x + 9, y - 0.5),
                     QPointF(x + 10, y + 1.8), QPointF(x - 2.5, y + 1.8)])
    p.setPen(pen(OUTLINE, 1.1))
    p.setBrush(color)
    p.drawPolygon(web)


def _paint_wing(p: QPainter, raised: float) -> None:
    p.save()
    p.translate(6, -42)          # shoulder
    p.rotate(105 * raised)       # folded along the back -> raised up and back
    wing = QPainterPath(QPointF(2, -2))
    wing.cubicTo(QPointF(-10, -9), QPointF(-28, -7), QPointF(-36, 4))
    wing.cubicTo(QPointF(-22, 12), QPointF(-4, 10), QPointF(2, -2))
    p.setPen(pen(OUTLINE, LINE_W))
    p.setBrush(WING)
    p.drawPath(wing)
    p.setPen(pen(WING_LINE, 1.2))
    p.setBrush(Qt.NoBrush)
    for (x1, y1), (x2, y2) in (((-18, 3), (-30, 4)), ((-14, 7), (-26, 8)), ((-21, -1), (-32, 1))):
        p.drawLine(QPointF(x1, y1), QPointF(x2, y2))
    p.restore()


def _paint_head(p: QPainter, pose: GoosePose, head: QPointF, facing: int) -> None:
    p.save()
    p.translate(head)
    p.rotate(pose.head_angle)

    p.save()                     # lower mandible hinges open
    p.translate(6, 1.5)
    p.rotate(30 * min(1.0, pose.beak_open))
    p.setPen(pen(OUTLINE, 1.2))
    p.setBrush(ORANGE_DARK)
    p.drawPolygon(QPolygonF([QPointF(0, -0.8), QPointF(11, 0.8), QPointF(0, 2.8)]))
    p.restore()

    p.setPen(pen(OUTLINE, 1.2))
    p.setBrush(ORANGE)
    p.drawPolygon(QPolygonF([QPointF(5, -4.5), QPointF(13, -2.5), QPointF(19, 0.8), QPointF(5, 2)]))
    p.drawEllipse(QPointF(6.5, -4.6), 2.4, 2.4)   # knob

    eye = QPointF(1.5, -2.8)
    lx, ly = pose.look
    lx *= facing
    if pose.eyes in ("open", "wide"):
        if pose.eyes == "wide":
            p.setPen(pen(OUTLINE, 1.0))
            p.setBrush(WHITE)
            p.drawEllipse(eye, 3.4, 3.4)
            r, k = 1.7, 0.9
        else:
            r, k = 2.0, 0.7
        pupil = QPointF(eye.x() + lx * k, eye.y() + ly * k)
        p.setPen(Qt.NoPen)
        p.setBrush(EYE)
        p.drawEllipse(pupil, r, r)
        p.setBrush(WHITE)
        p.drawEllipse(QPointF(pupil.x() - 0.6, pupil.y() - 0.7), 0.7, 0.7)
    else:
        path = QPainterPath()
        x, y = eye.x(), eye.y()
        if pose.eyes == "closed":
            path.moveTo(x - 2.6, y)
            path.quadTo(x, y + 2, x + 2.6, y)
        elif pose.eyes == "happy":
            path.moveTo(x - 2.6, y + 1)
            path.quadTo(x, y - 2.6, x + 2.6, y + 1)
        else:  # dizzy
            path.moveTo(x - 2.2, y - 2.2)
            path.lineTo(x + 2.2, y + 2.2)
            path.moveTo(x + 2.2, y - 2.2)
            path.lineTo(x - 2.2, y + 2.2)
        p.setBrush(Qt.NoBrush)
        p.setPen(pen(EYE, 1.5))
        p.drawPath(path)
    p.restore()

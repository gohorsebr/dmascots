"""Procedural goose renderer (side view): every frame is painted from a GoosePose.

Coordinate system (before the caller's scale): origin between the feet,
y grows downward, the goose faces +x. The painter is mirrored for facing=-1.

Body, tapered neck and head are unioned into one silhouette so the outline
flows around the whole bird; shading, wing and face are layered on top.
"""
from __future__ import annotations

import math

from PySide6.QtCore import QPointF, Qt
from PySide6.QtGui import QColor, QPainter, QPainterPath, QPolygonF

from ...art import paint_bubble, paint_stars, pen, polyline, stroke, tapered_curve
from .brain import GoosePose

FEATHER = QColor("#FCFCF8")
SHADE = QColor("#E4E4DB")
WING = QColor("#F1F1EA")
WING_SHADE = QColor("#DADAD0")
WING_LINE = QColor("#A8A89D")
OUTLINE = QColor("#343434")
ORANGE = QColor("#F59B2A")
ORANGE_DARK = QColor("#CF7417")
NAIL = QColor("#A85B12")
EYE = QColor("#151515")
WHITE = QColor("#FFFFFF")

LINE_W = 1.8
HEAD_R = 9.8
NECK_BASE = QPointF(12, -42)
NECK_BASE_W = 24.0              # thick where it flows into the breast...
NECK_TOP_W = 12.0               # ...slender under the head
PIVOT = QPointF(0, -54)         # grab point when dangling from the cursor
HIP_BACK, HIP_FRONT = QPointF(-7, -22), QPointF(1, -22)
LEG = 19.0

# Extents used by the window for layout / screen-edge collisions
HALF_WIDTH = 50.0
HEIGHT = 92.0
EYE_Y = 81.0


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
    body = None if head_only else _body_path(pose.tail)
    skull = QPainterPath()
    skull.addEllipse(head, HEAD_R, HEAD_R)
    # boolean union (not addPath): the pieces wind in different directions
    sil = _neck_path(head, pose.neck_bend).united(skull)
    if body is not None:
        sil = sil.united(body)

    if not head_only:
        for hip, angle, color in ((HIP_BACK, pose.leg_back, ORANGE_DARK), (HIP_FRONT, pose.leg_front, ORANGE)):
            a = math.radians(angle)
            foot = QPointF(hip.x() + LEG * math.sin(a), hip.y() + LEG * math.cos(a))
            stroke(p, polyline((hip.x(), hip.y()), (foot.x(), foot.y())), color, 3.6,
                   outline=OUTLINE, outline_w=1.1)
            _paint_foot(p, foot, color)

    p.setPen(Qt.NoPen)
    p.setBrush(FEATHER)
    p.drawPath(sil)
    if body is not None:                  # soft shading under the belly and tail
        p.save()
        p.setClipPath(body)
        p.setBrush(SHADE)
        p.drawEllipse(QPointF(-8, -16), 34, 10)
        p.drawEllipse(QPointF(-38, -36), 10, 9)
        p.restore()
    p.setPen(pen(OUTLINE, LINE_W))
    p.setBrush(Qt.NoBrush)
    p.drawPath(sil)

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
    """The neck: a cubic spine from the breast to the head, swept with a tapering width."""
    p0 = (NECK_BASE.x(), NECK_BASE.y())
    return tapered_curve(p0, (p0[0] + 6 + bend, p0[1] - 16),
                         (head.x() - 4 + bend * 0.5, head.y() + 14), (head.x(), head.y()),
                         NECK_BASE_W, NECK_TOP_W)


def _body_path(tail: float) -> QPainterPath:
    """Pear-shaped body: deep breast up front, belly, upturned tail at the back."""
    path = QPainterPath(QPointF(18, -50))
    path.cubicTo(QPointF(29, -42), QPointF(26, -22), QPointF(6, -18))        # breast
    path.cubicTo(QPointF(-10, -15), QPointF(-27, -21), QPointF(-33, -32))    # belly
    path.lineTo(QPointF(-44, -50 + 3 * tail))                                  # tail tip
    path.cubicTo(QPointF(-35, -51), QPointF(-26, -55), QPointF(-12, -55))    # back
    path.cubicTo(QPointF(0, -55), QPointF(10, -55), QPointF(18, -50))
    path.closeSubpath()
    return path


def _paint_foot(p: QPainter, ankle: QPointF, color: QColor) -> None:
    x, y = ankle.x(), ankle.y()
    web = QPainterPath(QPointF(x - 3, y - 1.4))
    web.lineTo(x + 4, y - 2.4)
    web.quadTo(x + 10, y - 1.8, x + 13.5, y + 0.2)       # outer toe
    web.quadTo(x + 11.5, y + 1.0, x + 12.5, y + 2.2)     # webbing scallop
    web.lineTo(x - 3.5, y + 2.2)
    web.closeSubpath()
    p.setPen(pen(OUTLINE, 1.1))
    p.setBrush(color)
    p.drawPath(web)
    p.setPen(pen(NAIL, 0.8))
    p.drawLine(QPointF(x + 1, y - 0.2), QPointF(x + 10, y + 0.6))


def _paint_wing(p: QPainter, raised: float) -> None:
    p.save()
    p.translate(2, -43)          # shoulder
    p.rotate(105 * raised)       # folded along the back -> raised up and back
    wing = QPainterPath(QPointF(4, -2))
    wing.cubicTo(QPointF(-8, -10), QPointF(-26, -9), QPointF(-38, 2))   # leading edge to tip
    wing.quadTo(QPointF(-33, 3), QPointF(-31, 7))                       # primaries...
    wing.quadTo(QPointF(-27, 5), QPointF(-24, 10))
    wing.quadTo(QPointF(-19, 7), QPointF(-15, 11))
    wing.cubicTo(QPointF(-6, 12), QPointF(2, 6), QPointF(4, -2))
    p.setPen(Qt.NoPen)
    p.setBrush(WING)
    p.drawPath(wing)
    p.save()
    p.setClipPath(wing)
    p.setBrush(WING_SHADE)
    p.drawEllipse(QPointF(-22, 12), 20, 6)
    p.restore()
    p.setPen(pen(WING_LINE, 1.1))
    p.setBrush(Qt.NoBrush)
    for (x1, y1), (x2, y2) in (((-12, 6), (-27, 4)), ((-9, 1), (-30, 0)), ((-16, 9), (-23, 8))):
        p.drawLine(QPointF(x1, y1), QPointF(x2, y2))
    p.setPen(pen(OUTLINE, LINE_W))
    p.drawPath(wing)
    p.restore()


def _paint_head(p: QPainter, pose: GoosePose, head: QPointF, facing: int) -> None:
    p.save()
    p.translate(head)
    p.rotate(pose.head_angle)

    p.save()                     # lower mandible hinges open
    p.translate(5.5, 1.8)
    p.rotate(30 * min(1.0, pose.beak_open))
    lower = QPainterPath(QPointF(0, -0.6))
    lower.quadTo(QPointF(7, 1.2), QPointF(13.5, 0.4))
    lower.lineTo(QPointF(13, 1.8))
    lower.quadTo(QPointF(6, 3.8), QPointF(0, 3.2))
    lower.closeSubpath()
    p.setPen(pen(OUTLINE, 1.2))
    p.setBrush(ORANGE_DARK)
    p.drawPath(lower)
    p.restore()

    p.setPen(pen(OUTLINE, 1.2))
    p.setBrush(ORANGE)
    p.drawEllipse(QPointF(5.6, -5.4), 2.7, 2.2)          # knob
    upper = QPainterPath(QPointF(4.5, -5.6))
    upper.cubicTo(QPointF(10, -5), QPointF(16, -2.6), QPointF(20, 1.2))   # ridge curving to the tip
    upper.quadTo(QPointF(19.6, 2.5), QPointF(17.5, 2.3))
    upper.cubicTo(QPointF(12, 2.2), QPointF(8, 2.5), QPointF(4.5, 2.7))
    upper.quadTo(QPointF(3, -1.5), QPointF(4.5, -5.6))
    upper.closeSubpath()
    p.drawPath(upper)
    p.setPen(Qt.NoPen)
    p.setBrush(NAIL)
    p.drawEllipse(QPointF(18.8, 1.3), 1.5, 1.1)          # nail at the tip
    p.drawEllipse(QPointF(10.5, -2.3), 1.0, 0.6)         # nostril

    eye = QPointF(1.2, -3.2)
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

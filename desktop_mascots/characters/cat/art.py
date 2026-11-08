"""Orange tabby painter."""
from __future__ import annotations

import math

from PySide6.QtCore import QPointF, Qt
from PySide6.QtGui import QColor, QPainter, QPainterPath, QPen, QPolygonF, QTransform

from ...art import pen, tapered_curve
from ...quadruped.art import Geometry, QuadArt, circle
from ...quadruped.brain import QuadPose
from .brain import CAT_RIG

STRIPE = QColor("#B9622A")
CREAM = QColor("#F7D9AE")
PINK = QColor("#EE9AA2")
MOUTH = QColor("#5A2226")
IRIS = QColor("#8CC63F")
WHISKER = QColor("#FFF6E8")

EAR_BACK = ((-6.0, -4.0), (-4.5, -15.0), (1.0, -7.0))
EAR_FRONT = ((-1.0, -7.0), (3.0, -16.0), (6.5, -5.0))


def _tri(pts, inset: float = 0.0) -> QPolygonF:
    cx = sum(x for x, _ in pts) / 3
    cy = sum(y for _, y in pts) / 3
    return QPolygonF([QPointF(x + (cx - x) * inset, y + (cy - y) * inset) for x, y in pts])


class CatArt(QuadArt):
    rig = CAT_RIG
    coat = QColor("#E8964A")
    shade = QColor("#C97A36")
    outline = QColor("#4A2A14")
    chest_r = 8.0
    hip_r = 8.5
    neck_w = (11.0, 9.0)
    front_w = (5.4, 3.6, 3.4)
    hind_w = (7.0, 3.8, 3.4)
    tail_len = 30.0

    def _ear_transform(self, pose: QuadPose, base: tuple[float, float]) -> QTransform:
        back = -35 * max(0.0, -pose.ears)      # flattened when annoyed / scared
        return QTransform().translate(*base).rotate(back).translate(-base[0], -base[1])

    def head_shape(self, pose: QuadPose, g: Geometry) -> QPainterPath:
        head = QPainterPath()
        head.addEllipse(QPointF(0, 0), 9.0, 8.0)
        head = head.united(circle(QPointF(7.0, 3.0), 4.4))
        for ear in (EAR_BACK, EAR_FRONT):
            path = QPainterPath()
            path.addPolygon(_tri(ear))
            path.closeSubpath()
            head = head.united(self._ear_transform(pose, ear[0]).map(path))
        return self.head_transform(g).map(head)

    def tail_path(self, pose: QuadPose, g: Geometry):
        a = math.radians(pose.tail + 0.3 * pose.tail_wag)
        L = self.tail_len
        r = g.tail_root
        dx, dy = -math.cos(a), -math.sin(a)      # back, and up by the tail angle
        ux, uy = -dy * -1, dx * -1               # perpendicular, toward "up" from the tail
        curl = pose.tail_curl
        p1 = (r.x() + dx * L * 0.4, r.y() + dy * L * 0.4)
        p2 = (r.x() + dx * L * 0.85 + ux * L * 0.15 * curl, r.y() + dy * L * 0.85 + uy * L * 0.15 * curl)
        tip = (r.x() + dx * L * 0.85 + ux * L * 0.5 * curl, r.y() + dy * L * 0.85 + uy * L * 0.5 * curl)
        return tapered_curve((r.x(), r.y()), p1, p2, tip, 5.5, 4.2, steps=20, power=1.0), \
            [(r.x(), r.y()), p1, p2, tip]

    def paint_tail(self, p: QPainter, pose: QuadPose, g: Geometry) -> None:
        path, ctrl = self.tail_path(pose, g)
        p.setPen(Qt.NoPen)
        p.setBrush(self.coat)
        p.drawPath(path)
        p.save()                                  # tabby rings: a dashed stroke along the spine
        p.setClipPath(path)
        spine = QPainterPath(QPointF(*ctrl[0]))
        spine.cubicTo(QPointF(*ctrl[1]), QPointF(*ctrl[2]), QPointF(*ctrl[3]))
        rings = QPen(STRIPE, 8)
        rings.setDashPattern([0.45, 0.55])
        rings.setDashOffset(-0.6)
        p.setPen(rings)
        p.setBrush(Qt.NoBrush)
        p.drawPath(spine)
        p.restore()
        p.setPen(pen(self.outline, self.line_w))
        p.setBrush(Qt.NoBrush)
        p.drawPath(path)

    def paint_markings(self, p: QPainter, pose: QuadPose, g: Geometry) -> None:
        h, s = g.hip, g.shoulder
        p.setPen(Qt.NoPen)
        p.setBrush(CREAM)
        p.drawEllipse(QPointF(s.x() + 4, s.y() + 6), 6, 5)        # chest
        p.setPen(pen(STRIPE, 2.3))
        dx, dy = s.x() - h.x(), s.y() - h.y()
        n = math.hypot(dx, dy) or 1.0
        nx, ny = dy / n, -dx / n                                     # body normal, toward the back
        for f in (0.05, 0.28, 0.5, 0.72):
            cx, cy = h.x() + dx * f, h.y() + dy * f
            r = self.hip_r + (self.chest_r - self.hip_r) * f
            stripe = QPainterPath(QPointF(cx + nx * (r + 1), cy + ny * (r + 1)))
            stripe.quadTo(cx + nx * r * 0.3 + 2.5, cy + ny * r * 0.3, cx - nx * r * 0.2, cy - ny * r * 0.2)
            p.drawPath(stripe)
        p.save()
        p.setTransform(self.head_transform(g), True)
        p.setPen(Qt.NoPen)
        p.setBrush(CREAM)
        p.drawEllipse(QPointF(8.0, 4.2), 5.0, 3.4)                  # muzzle
        p.setPen(pen(STRIPE, 1.6))
        for x in (-3.0, 0.0, 3.0):                                   # forehead "M"
            p.drawLine(QPointF(x, -7.5), QPointF(x + 0.8, -4.5))
        p.restore()

    def paint_head(self, p: QPainter, pose: QuadPose, g: Geometry, facing: int) -> None:
        p.save()
        p.setTransform(self.head_transform(g), True)
        p.setPen(Qt.NoPen)
        p.setBrush(PINK)
        for ear in (EAR_BACK, EAR_FRONT):
            p.save()
            p.setTransform(self._ear_transform(pose, ear[0]), True)
            p.drawPolygon(_tri(ear, 0.38))
            p.restore()
        if pose.mouth > 0.05:
            hinge = QPointF(6.0, 5.0)
            t = QTransform().translate(hinge.x(), hinge.y()).rotate(30 * pose.mouth)
            jaw = QPolygonF([QPointF(0, 0), QPointF(5.5, 0.3), QPointF(5, 2.4), QPointF(0, 2.6)])
            p.setBrush(MOUTH)
            p.drawPolygon(QPolygonF([hinge, QPointF(11.5, 4.6), t.map(QPointF(5.5, 0.3))]))
            p.setPen(pen(self.outline, 1.0))
            p.setBrush(CREAM)
            p.drawPolygon(t.map(jaw))
            if pose.tongue:
                p.setPen(Qt.NoPen)
                p.setBrush(PINK)
                p.drawEllipse(QPointF(11.0, 6.2), 2.0, 1.4)
        else:
            mouth = QPainterPath(QPointF(11.0, 3.2))
            mouth.quadTo(10.5, 5.6, 8.6, 5.4)
            mouth.moveTo(11.0, 3.2)
            mouth.quadTo(11.6, 5.8, 13.0, 5.0)
            p.setPen(pen(self.outline, 1.0))
            p.setBrush(Qt.NoBrush)
            p.drawPath(mouth)
        p.setPen(pen(self.outline, 0.8))
        p.setBrush(PINK)
        p.drawPolygon(QPolygonF([QPointF(10.0, 1.0), QPointF(12.0, 1.0), QPointF(11.2, 2.8)]))   # nose
        self.paint_eye(p, pose, QPointF(4.0, -2.2), facing, r=2.3, iris=IRIS,
                       slit=pose.eyes != "wide", light=self.outline)
        p.setPen(pen(WHISKER, 0.7))
        for y0, y1 in ((3.6, 1.6), (4.4, 4.2), (5.2, 6.8)):
            p.drawLine(QPointF(10.0, y0), QPointF(19.5, y1))
        p.restore()

    def paint_eye(self, p, pose, at, facing, r=2.2, iris=None, slit=False, light=None):
        if pose.eyes in ("open", "wide"):   # almond shape: a lid line over the eye
            super().paint_eye(p, pose, at, facing, r, iris, slit, light)
            p.setPen(pen(self.outline, 1.1))
            p.setBrush(Qt.NoBrush)
            lid = QPainterPath(QPointF(at.x() - r * 1.4, at.y() + 0.3))
            lid.quadTo(at.x(), at.y() - r * 1.5, at.x() + r * 1.4, at.y() - 0.2)
            p.drawPath(lid)
        else:
            super().paint_eye(p, pose, at, facing, r, iris, slit, light)

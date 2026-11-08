"""Bay horse painter: brown coat, black mane, tail and lower legs, white star and stripe."""
from __future__ import annotations

import math

from PySide6.QtCore import QPointF, QRectF, Qt
from PySide6.QtGui import QColor, QPainter, QPainterPath, QPolygonF, QTransform

from ...art import pen, tapered_curve
from ...quadruped.art import Geometry, QuadArt, circle, straight
from ...quadruped.brain import QuadPose
from .brain import HORSE_RIG

MANE = QColor("#1E1512")
MANE_LINE = QColor("#46352C")
MUZZLE = QColor("#5E3219")
WHITE = QColor("#F6F2EA")
MOUTH = QColor("#3A1A14")

EARS = (((-6.0, -6.0), (-13.0, -19.0), (-1.5, -9.0)),
        ((-2.0, -8.0), (-7.5, -21.0), (3.0, -8.5)))


class HorseArt(QuadArt):
    rig = HORSE_RIG
    coat = QColor("#8B4A22")
    shade = QColor("#6C3618")
    outline = QColor("#2A140A")
    chest_r = 17.0
    hip_r = 17.5
    neck_w = (34.0, 18.0)
    front_w = (14.0, 7.0, 6.5)
    head_scale = 1.18
    hind_w = (17.0, 7.5, 6.5)
    foot = "hoof"
    hoof_color = QColor("#2B2522")
    line_w = 1.8

    def head_transform(self, g: Geometry) -> QTransform:
        return super().head_transform(g).scale(self.head_scale, self.head_scale)

    def head_shape(self, pose: QuadPose, g: Geometry) -> QPainterPath:
        head = circle(QPointF(0, 0), 10.5).united(circle(QPointF(2.0, 5.0), 9.5))
        head = head.united(straight(QPointF(4, 2), QPointF(25, 3.5), 18.0, 12.5))
        head = head.united(circle(QPointF(25.0, 4.0), 6.6))
        for ear in EARS:
            path = QPainterPath()
            path.addPolygon(QPolygonF([QPointF(*pt) for pt in ear]))
            path.closeSubpath()
            back = -25 * max(0.0, -pose.ears) + 8 * max(0.0, pose.ears)
            t = QTransform().translate(*ear[0]).rotate(back).translate(-ear[0][0], -ear[0][1])
            head = head.united(t.map(path))
        return self.head_transform(g).map(head)

    def paint_tail(self, p: QPainter, pose: QuadPose, g: Geometry) -> None:
        r = g.tail_root
        lift = math.radians(pose.tail)
        swish = 12 * math.sin(math.radians(pose.tail_wag) * 2)
        up = 22 * math.sin(lift)
        p0 = (r.x(), r.y())
        p1 = (r.x() - 13, r.y() - 2 - up * 0.6)
        p2 = (r.x() - 22 + swish * 0.5, r.y() + 18 - up)
        tip = (r.x() - 16 + swish, r.y() + 46 - up * 1.4)
        tail = tapered_curve(p0, p1, p2, tip, 9.0, 13.0, steps=20, power=1.0)
        p.setPen(pen(self.outline, self.line_w))
        p.setBrush(MANE)
        p.drawPath(tail)
        p.setPen(pen(MANE_LINE, 1.0))
        for off in (-3.0, 0.0, 3.0):                 # a few strands of hair
            strand = QPainterPath(QPointF(p1[0], p1[1] + 4))
            strand.cubicTo(QPointF(p1[0] - 4 + off, p1[1] + 10), QPointF(p2[0] + off, p2[1] + 6),
                           QPointF(tip[0] + off * 1.5, tip[1] - 6))
            p.drawPath(strand)

    def paint_markings(self, p: QPainter, pose: QuadPose, g: Geometry) -> None:
        p.setPen(Qt.NoPen)
        p.setBrush(MANE)
        p.drawRect(QRectF(-300, -21, 600, 30))                  # black "points" on the lower legs
        p.save()
        p.setTransform(self.head_transform(g), True)
        p.setBrush(MUZZLE)
        p.drawEllipse(QPointF(25.0, 4.5), 8.0, 7.5)
        p.setBrush(WHITE)
        p.drawEllipse(QPointF(2.0, -6.0), 3.4, 2.6)              # star
        p.drawPath(straight(QPointF(6, -5.5), QPointF(21, -1.5), 3.2, 2.0))   # stripe
        p.restore()

    def paint_far_markings(self, p: QPainter, pose: QuadPose, g: Geometry) -> None:
        p.setPen(Qt.NoPen)
        p.setBrush(MANE)
        p.drawRect(QRectF(-300, -21, 600, 30))

    def paint_over(self, p: QPainter, pose: QuadPose, g: Geometry) -> None:
        """Mane along the crest of the neck (following its curve), plus a forelock."""
        spine = self.neck_spine(g)
        (x0, y0), (x3, y3) = spine[0], spine[3]
        n = math.hypot(x3 - x0, y3 - y0) or 1.0
        ux, uy = (y3 - y0) / n, -(x3 - x0) / n        # perpendicular to the neck...
        if ux > 0:
            ux, uy = -ux, -uy                          # ...on the crest (back/top) side
        offs = (self.neck_w[0] * 0.42, self.neck_w[0] * 0.34, self.neck_w[1] * 0.45, self.neck_w[1] * 0.35)
        ctrl = [(x + ux * o, y + uy * o) for (x, y), o in zip(spine, offs)]
        ctrl[3] = (ctrl[3][0] - 2, ctrl[3][1] - 3)
        mane = tapered_curve(*ctrl, 10.0, 6.0, steps=16, power=1.0)
        p.setPen(pen(self.outline, 1.2))
        p.setBrush(MANE)
        p.drawPath(mane)
        p.setPen(pen(MANE_LINE, 0.9))
        for f in (0.2, 0.4, 0.6, 0.8):
            u = 1 - f
            x = u ** 3 * ctrl[0][0] + 3 * u * u * f * ctrl[1][0] + 3 * u * f * f * ctrl[2][0] + f ** 3 * ctrl[3][0]
            y = u ** 3 * ctrl[0][1] + 3 * u * u * f * ctrl[1][1] + 3 * u * f * f * ctrl[2][1] + f ** 3 * ctrl[3][1]
            p.drawLine(QPointF(x + ux * 3, y + uy * 3), QPointF(x - ux * 3, y - uy * 3))

    def paint_head(self, p: QPainter, pose: QuadPose, g: Geometry, facing: int) -> None:
        p.save()
        p.setTransform(self.head_transform(g), True)
        forelock = QPolygonF([QPointF(-7, -9), QPointF(-1, -10), QPointF(3, -4), QPointF(-1, -2.5)])
        p.setPen(pen(self.outline, 1.0))
        p.setBrush(MANE)
        p.drawPolygon(forelock)
        p.setPen(Qt.NoPen)
        p.setBrush(QColor("#1A0E09"))
        p.drawEllipse(QPointF(28.0, 0.8), 1.5, 2.3)              # nostril
        if pose.mouth > 0.05:
            lip = QTransform().translate(19, 8).rotate(22 * pose.mouth)
            p.setBrush(MOUTH)
            p.drawPolygon(QPolygonF([QPointF(19, 8), QPointF(30.5, 7), lip.map(QPointF(10, 1))]))
            p.setPen(pen(self.outline, 1.1))
            p.setBrush(MUZZLE)
            p.drawPolygon(lip.map(QPolygonF([QPointF(0, 0), QPointF(10, 1), QPointF(9.5, 3.5), QPointF(0, 3.5)])))
        else:
            p.setPen(pen(self.outline, 1.1))
            p.drawLine(QPointF(30.0, 7.5), QPointF(22.0, 8.5))
        self.paint_eye(p, pose, QPointF(3.0, -3.0), facing, r=2.4, light=WHITE)
        p.restore()

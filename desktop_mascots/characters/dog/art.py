"""Dog painters: a yellow labrador and a black-and-white border collie."""
from __future__ import annotations

import math

from PySide6.QtCore import QPointF, QRectF, Qt
from PySide6.QtGui import QColor, QPainter, QPainterPath, QPolygonF, QTransform

from ...art import pen, tapered_curve
from ...quadruped.art import Geometry, QuadArt, circle
from ...quadruped.brain import QuadPose
from .brain import COLLIE_RIG, LAB_RIG

NOSE = QColor("#2E2220")
MOUTH = QColor("#4A1E1E")
TONGUE = QColor("#E8707A")


def _ellipse(c: QPointF, rx: float, ry: float) -> QPainterPath:
    path = QPainterPath()
    path.addEllipse(c, rx, ry)
    return path


class DogArt(QuadArt):
    neck_w = (17.0, 13.0)
    front_w = (8.0, 5.0, 4.6)
    hind_w = (10.0, 5.5, 4.6)
    muzzle = (10.0, 3.0, 9.5, 6.2)     # local centre x, y, rx, ry
    nose_at = QPointF(19.0, 0.8)
    tail_len = 20.0
    tail_w = (6.0, 2.5)
    tail_base = 10.0                  # degrees above horizontal, pointing back

    def head_shape(self, pose: QuadPose, g: Geometry) -> QPainterPath:
        mx, my, rx, ry = self.muzzle
        local = circle(QPointF(0, 0), 9.5).united(_ellipse(QPointF(mx, my), rx, ry))
        return self.head_transform(g).map(local.united(self.ear_silhouette(pose)))

    def ear_silhouette(self, pose: QuadPose) -> QPainterPath:
        return QPainterPath()

    def tail_path(self, pose: QuadPose, g: Geometry) -> tuple[QPainterPath, QPointF]:
        a = math.radians(self.tail_base + pose.tail + 0.3 * pose.tail_wag)
        length = self.tail_len * (1 - 0.3 * abs(math.sin(math.radians(pose.tail_wag))))  # wagging foreshortens
        r = g.tail_root
        dx, dy = -math.cos(a), -math.sin(a)
        tip = (r.x() + dx * length, r.y() + dy * length)
        p1 = (r.x() + dx * length * 0.35, r.y() + dy * length * 0.35)
        p2 = (r.x() + dx * length * 0.75, r.y() + dy * length * 0.75 - length * 0.08)
        return tapered_curve((r.x(), r.y()), p1, p2, tip, *self.tail_w), QPointF(*tip)

    def paint_tail(self, p: QPainter, pose: QuadPose, g: Geometry) -> None:
        path, _ = self.tail_path(pose, g)
        p.setPen(pen(self.outline, self.line_w))
        p.setBrush(self.coat)
        p.drawPath(path)

    def paint_head(self, p: QPainter, pose: QuadPose, g: Geometry, facing: int) -> None:
        p.save()
        p.setTransform(self.head_transform(g), True)
        self.paint_mouth(p, pose)
        self.paint_ear(p, pose)
        self.paint_eye(p, pose, QPointF(3.5, -3.2), facing, r=2.0)
        n = self.nose_at
        p.setPen(Qt.NoPen)
        p.setBrush(NOSE)
        p.drawEllipse(n, 2.8, 2.3)
        p.setBrush(QColor(255, 255, 255, 120))
        p.drawEllipse(QPointF(n.x() - 0.8, n.y() - 0.9), 0.9, 0.6)
        p.restore()

    def paint_ear(self, p: QPainter, pose: QuadPose) -> None:
        pass

    def paint_mouth(self, p: QPainter, pose: QuadPose) -> None:
        tip = self.nose_at.x() - 0.5
        if pose.mouth > 0.05:
            hinge = QPointF(7.0, 5.0)
            jaw = QPainterPath(QPointF(0, 0))
            jaw.lineTo(tip - 8, 0.4)
            jaw.quadTo(tip - 7, 3.2, tip - 9.5, 3.6)
            jaw.lineTo(0, 3.4)
            jaw.closeSubpath()
            t = QTransform().translate(hinge.x(), hinge.y()).rotate(26 * pose.mouth)
            jaw = t.map(jaw)
            gape = QPolygonF([hinge, QPointF(tip - 1, 4.6), t.map(QPointF(tip - 8, 0.4))])
            p.setPen(Qt.NoPen)
            p.setBrush(MOUTH)
            p.drawPolygon(gape)
            p.setPen(pen(self.outline, 1.1))
            p.setBrush(self.muzzle_color())
            p.drawPath(jaw)
            if pose.tongue:
                tongue = tapered_curve((11.5, 6.0), (12.5, 8.5), (13.5, 11.0 + 3 * pose.mouth),
                                       (13.0, 13.0 + 4 * pose.mouth), 4.0, 3.4, steps=8)
                p.setPen(pen(QColor("#B04652"), 0.9))
                p.setBrush(TONGUE)
                p.drawPath(tongue)
        else:
            mouth = QPainterPath(QPointF(tip - 1, 5.0))
            mouth.quadTo(tip - 6, 6.6, 9.0, 5.4)
            mouth.quadTo(8.0, 5.0, 8.2, 4.2)
            p.setPen(pen(self.outline, 1.1))
            p.setBrush(Qt.NoBrush)
            p.drawPath(mouth)

    def muzzle_color(self) -> QColor:
        return self.coat


class LabradorArt(DogArt):
    rig = LAB_RIG
    coat = QColor("#E6BE77")
    shade = QColor("#C99E57")
    ear = QColor("#CFA05A")
    outline = QColor("#5B3E1D")
    chest_r = 12.0
    hip_r = 11.0

    def paint_ear(self, p: QPainter, pose: QuadPose) -> None:
        """Floppy ear hanging over the side of the head."""
        p.save()
        p.translate(-2, -6)
        p.rotate(-14 * pose.ears)
        ear = QPainterPath(QPointF(-1, -1))
        ear.cubicTo(-7, 1, -8, 12, -4, 15)
        ear.quadTo(0, 16, 3, 11)
        ear.cubicTo(3, 5, 2, 0, -1, -1)
        p.setPen(pen(self.outline, 1.2))
        p.setBrush(self.ear)
        p.drawPath(ear)
        p.restore()

    def paint_markings(self, p: QPainter, pose: QuadPose, g: Geometry) -> None:
        p.setPen(Qt.NoPen)
        p.setBrush(QColor("#F0D29A"))   # lighter chest and belly
        p.drawEllipse(QPointF(g.shoulder.x() + 3, g.shoulder.y() + 7), 9, 6)


class CollieArt(DogArt):
    rig = COLLIE_RIG
    coat = QColor("#232126")
    shade = QColor("#151417")
    outline = QColor("#0B0A0C")
    white = QColor("#F3F1EC")
    chest_r = 10.5
    hip_r = 9.5
    neck_w = (17.0, 12.0)
    muzzle = (11.0, 3.2, 10.5, 5.2)
    nose_at = QPointF(21.0, 1.2)
    tail_len = 22.0
    tail_w = (7.0, 6.0)               # bushy
    tail_base = -25.0                 # carried low

    def ear_silhouette(self, pose: QuadPose) -> QPainterPath:
        """Semi-pricked ear, tip folded forward."""
        ear = QPainterPath()
        ear.addPolygon(QPolygonF([QPointF(-6, -5), QPointF(-6.5, -14), QPointF(-3.5, -19),
                                  QPointF(0.5, -16.5), QPointF(2, -6)]))
        ear.closeSubpath()
        return QTransform().translate(-2, -5).rotate(-20 * min(0.0, pose.ears)).translate(2, 5).map(ear)

    def muzzle_color(self) -> QColor:
        return self.white

    def paint_tail(self, p: QPainter, pose: QuadPose, g: Geometry) -> None:
        path, tip = self.tail_path(pose, g)
        p.setPen(Qt.NoPen)
        p.setBrush(self.coat)
        p.drawPath(path)
        p.save()
        p.setClipPath(path)
        p.setBrush(self.white)
        p.drawEllipse(tip, 6, 6)             # white tail tip
        p.restore()
        p.setPen(pen(self.outline, self.line_w))
        p.setBrush(Qt.NoBrush)
        p.drawPath(path)

    def paint_markings(self, p: QPainter, pose: QuadPose, g: Geometry) -> None:
        p.setPen(Qt.NoPen)
        p.setBrush(self.white)
        nb = g.neck_base
        p.drawEllipse(QPointF(nb.x() + 5, nb.y() + 10), 9, 11)            # ruff and chest
        p.drawRect(QRectF(-200, -8.5, 400, 20))                           # socks
        p.save()
        p.setTransform(self.head_transform(g), True)
        p.drawEllipse(QPointF(12, 5), 11, 4.2)                            # white muzzle
        blaze = QPolygonF([QPointF(-1, -10), QPointF(2.5, -10), QPointF(9, -1), QPointF(4, 1)])
        p.drawPolygon(blaze)
        p.restore()

    def paint_far_markings(self, p: QPainter, pose: QuadPose, g: Geometry) -> None:
        p.setPen(Qt.NoPen)
        p.setBrush(QColor("#CFCCC6"))
        p.drawRect(QRectF(-200, -8.5, 400, 20))

    def paint_eye(self, p, pose, at, facing, r=2.2, iris=None, slit=False, light=None):
        super().paint_eye(p, pose, at, facing, r=r, iris=QColor("#7A4A22"), light=QColor("#E8E4DC"))

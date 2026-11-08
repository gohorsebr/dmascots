"""Painter for the four-legged rig. Species subclass QuadArt and fill in the head,
tail and markings; the body, neck and legs (solved with two-bone IK) are shared.

Coordinates (before the caller's scale): origin between the feet, y down,
the animal faces +x. The painter is mirrored for facing=-1.
"""
from __future__ import annotations

import math
from dataclasses import dataclass

from PySide6.QtCore import QPointF, Qt
from PySide6.QtGui import QColor, QPainter, QPainterPath, QPolygonF, QTransform

from ..art import paint_bubble, paint_stars, paint_zzz, pen, tapered_curve
from .brain import FF, FN, HF, HN, QuadPose, Rig


@dataclass
class Geometry:
    """Where everything ended up this frame (body coordinates)."""
    shoulder: QPointF
    hip: QPointF
    neck_base: QPointF
    head: QPointF
    head_angle: float
    tail_root: QPointF
    legs: list           # per foot: (joint, knee, foot) QPointFs


def _pt(t: tuple[float, float]) -> QPointF:
    return QPointF(t[0], t[1])


def ik(joint: QPointF, target: QPointF, a: float, b: float, bend: int) -> tuple[QPointF, QPointF]:
    """Two-bone IK: returns (knee, reachable foot). bend -1 = knee forward, +1 = backward."""
    dx, dy = target.x() - joint.x(), target.y() - joint.y()
    d0 = math.hypot(dx, dy) or 1e-6
    d = min(max(d0, abs(a - b) + 0.01), a + b - 0.01)
    ux, uy = dx / d0, dy / d0
    foot = QPointF(joint.x() + ux * d, joint.y() + uy * d)
    theta = math.atan2(uy, ux)
    alpha = math.acos(max(-1.0, min(1.0, (a * a + d * d - b * b) / (2 * a * d))))
    knee = QPointF(joint.x() + a * math.cos(theta + bend * alpha), joint.y() + a * math.sin(theta + bend * alpha))
    return knee, foot


def straight(a: QPointF, b: QPointF, w0: float, w1: float) -> QPainterPath:
    """A straight tapered segment (bones, muzzles)."""
    p0, p3 = (a.x(), a.y()), (b.x(), b.y())
    p1 = (p0[0] + (p3[0] - p0[0]) / 3, p0[1] + (p3[1] - p0[1]) / 3)
    p2 = (p0[0] + (p3[0] - p0[0]) * 2 / 3, p0[1] + (p3[1] - p0[1]) * 2 / 3)
    return tapered_curve(p0, p1, p2, p3, w0, w1, steps=4, power=1.0)


def circle(c: QPointF, r: float) -> QPainterPath:
    path = QPainterPath()
    path.addEllipse(c, r, r)
    return path


class QuadArt:
    rig: Rig
    coat = QColor("#888888")
    shade = QColor("#666666")        # far-side legs
    outline = QColor("#222222")
    chest_r = 10.0
    hip_r = 9.0
    neck_w = (14.0, 10.0)
    front_w = (8.0, 5.0, 4.0)        # leg widths: top, knee, foot
    hind_w = (9.0, 5.0, 4.0)
    foot = "paw"                     # paw | hoof
    paw_color = None                 # None = coat
    hoof_color = QColor("#2A2420")
    line_w = 1.6
    far_dx = 3.0                     # far legs sit a little behind/ahead for depth

    # ----------------------------------------------------------- geometry
    def geometry(self, pose: QuadPose) -> Geometry:
        r = self.rig
        s = QPointF(r.shoulder[0] + pose.shoulder[0], r.shoulder[1] + pose.shoulder[1])
        h = QPointF(r.hip[0] + pose.hip[0], r.hip[1] + pose.hip[1])
        legs = []
        for i, (dx, lift, hanging) in enumerate(pose.feet):
            front = i in (FN, FF)
            joint = QPointF((s if front else h).x() + (self.far_dx if i == FF else -self.far_dx if i == HF else 0),
                            (s if front else h).y())
            a, b = r.front_leg if front else r.hind_leg
            rest_x = (r.shoulder if front else r.hip)[0] + (joint.x() - (s if front else h).x())
            if hanging:
                target = QPointF(joint.x() + dx, joint.y() + (a + b) * 0.97 - lift)
            else:
                target = QPointF(rest_x + dx, r.foot_y - lift)
            knee, foot = ik(joint, target, a, b, -1 if front else 1)
            legs.append((joint, knee, foot))
        return Geometry(
            shoulder=s, hip=h,
            neck_base=QPointF(s.x() + r.neck_base[0], s.y() + r.neck_base[1]),
            head=QPointF(r.head[0] + pose.head[0], r.head[1] + pose.head[1]),
            head_angle=r.head_angle + pose.head_angle,
            tail_root=QPointF(h.x() + r.tail_root[0], h.y() + r.tail_root[1]),
            legs=legs)

    def head_transform(self, g: Geometry) -> QTransform:
        return QTransform().translate(g.head.x(), g.head.y()).rotate(g.head_angle)

    # ----------------------------------------------------------- shapes
    def body_shape(self, g: Geometry) -> QPainterPath:
        return straight(g.hip, g.shoulder, 2 * self.hip_r, 2 * self.chest_r) \
            .united(circle(g.hip, self.hip_r)).united(circle(g.shoulder, self.chest_r))

    def neck_spine(self, g: Geometry) -> list[tuple[float, float]]:
        """Bezier control points of the neck, neck base -> head."""
        nb, hd = g.neck_base, g.head
        return [(nb.x(), nb.y()),
                (nb.x() + (hd.x() - nb.x()) * 0.25, nb.y() + (hd.y() - nb.y()) * 0.55),
                (hd.x() - (hd.x() - nb.x()) * 0.3, hd.y() + (nb.y() - hd.y()) * 0.2),
                (hd.x(), hd.y())]

    def neck_shape(self, g: Geometry) -> QPainterPath:
        return tapered_curve(*self.neck_spine(g), *self.neck_w)

    def leg_shape(self, leg, widths) -> QPainterPath:
        joint, knee, foot = leg
        top, mid, low = widths
        shape = straight(joint, knee, top, mid).united(straight(knee, foot, mid, low))
        return shape.united(circle(knee, mid / 2))

    def foot_shape(self, foot: QPointF) -> QPainterPath:
        path = QPainterPath()
        if self.foot == "hoof":
            path.addPolygon(QPolygonF([QPointF(foot.x() - 3.5, foot.y() - 5), QPointF(foot.x() + 3.5, foot.y() - 5),
                                       QPointF(foot.x() + 5, foot.y()), QPointF(foot.x() - 4, foot.y())]))
            path.closeSubpath()
        else:
            path.addEllipse(QPointF(foot.x() + 1.5, foot.y() - 0.2), self.front_w[2] * 0.75, self.front_w[2] * 0.48)
        return path

    # ----------------------------------------------------------- species hooks
    def head_shape(self, pose: QuadPose, g: Geometry) -> QPainterPath:
        return self.head_transform(g).map(circle(QPointF(0, 0), 8))

    def paint_tail(self, p: QPainter, pose: QuadPose, g: Geometry) -> None:
        pass

    def paint_markings(self, p: QPainter, pose: QuadPose, g: Geometry) -> None:
        """Called clipped to the silhouette (patches, stripes, socks)."""

    def paint_far_markings(self, p: QPainter, pose: QuadPose, g: Geometry) -> None:
        """Called clipped to the far legs."""

    def paint_over(self, p: QPainter, pose: QuadPose, g: Geometry) -> None:
        """After the outline (manes, collars)."""

    def paint_head(self, p: QPainter, pose: QuadPose, g: Geometry, facing: int) -> None:
        pass

    # ----------------------------------------------------------- main
    def paint(self, p: QPainter, pose: QuadPose, facing: int = 1, head_only: bool = False) -> None:
        p.save()
        p.setRenderHint(QPainter.Antialiasing, True)
        g = self.geometry(pose)
        p.translate(0, pose.bob)
        p.scale(1.0 + (1.0 - pose.squash) * 0.6, pose.squash)
        p.scale(facing, 1)
        if pose.swing:
            pivot = QPointF(g.shoulder.x() - 4, g.shoulder.y() - self.chest_r)  # the scruff
            p.translate(pivot)
            p.rotate(pose.swing)
            p.translate(-pivot)
        p.rotate(pose.lean)

        sil = self.neck_shape(g).united(self.head_shape(pose, g))
        if not head_only:
            self.paint_tail(p, pose, g)
            far = QPainterPath()
            for i in (HF, FF):
                far = far.united(self.leg_shape(g.legs[i], self.front_w if i == FF else self.hind_w))
                if self.foot == "paw":
                    far = far.united(self.foot_shape(g.legs[i][2]))
            p.setPen(Qt.NoPen)
            p.setBrush(self.shade)
            p.drawPath(far)
            p.save()
            p.setClipPath(far)
            self.paint_far_markings(p, pose, g)
            p.restore()
            p.setPen(pen(self.outline, self.line_w))
            p.setBrush(Qt.NoBrush)
            p.drawPath(far)
            if self.foot == "hoof":
                self._paint_hooves(p, g, (HF, FF), self.hoof_color.darker(115))
            sil = sil.united(self.body_shape(g))
            for i in (HN, FN):
                sil = sil.united(self.leg_shape(g.legs[i], self.front_w if i == FN else self.hind_w))
                if self.foot == "paw":
                    sil = sil.united(self.foot_shape(g.legs[i][2]))

        p.setPen(Qt.NoPen)
        p.setBrush(self.coat)
        p.drawPath(sil)
        p.save()
        p.setClipPath(sil)
        self.paint_markings(p, pose, g)
        p.restore()
        p.setPen(pen(self.outline, self.line_w))
        p.setBrush(Qt.NoBrush)
        p.drawPath(sil)
        if not head_only and self.foot == "hoof":
            self._paint_hooves(p, g, (HN, FN), self.hoof_color)
        if not head_only:
            self.paint_over(p, pose, g)
        self.paint_head(p, pose, g, facing)
        if pose.stars is not None:
            paint_stars(p, g.head.x(), g.head.y() - 18, pose.stars, rx=17, ry=4)
        if pose.zzz is not None and not head_only:
            paint_zzz(p, g.head.x() + 4, g.head.y() - 12, pose.zzz, facing)
        p.restore()

        if pose.bubble and not head_only:
            top = min(g.head.y(), g.shoulder.y()) - 16
            paint_bubble(p, pose.bubble, facing * g.head.x(), top + pose.bob, facing, outline=self.outline)

    def _paint_hooves(self, p: QPainter, g: Geometry, which, color: QColor) -> None:
        for i in which:
            p.setPen(pen(self.outline, 1.2))
            p.setBrush(color)
            p.drawPath(self.foot_shape(g.legs[i][2]))

    # ----------------------------------------------------------- face helpers
    def paint_eye(self, p: QPainter, pose: QuadPose, at: QPointF, facing: int, r: float = 2.2,
                  iris: QColor | None = None, slit: bool = False, light: QColor | None = None) -> None:
        """Eye in head-local coordinates (painter already rotated to the head)."""
        lx, ly = pose.look
        lx *= facing
        stroke = light or self.outline
        if pose.eyes in ("open", "wide"):
            wide = pose.eyes == "wide"
            rr = r * (1.25 if wide else 1.0)
            if iris is not None:
                p.setPen(pen(self.outline, 0.9))
                p.setBrush(iris)
                p.drawEllipse(at, rr * 1.15, rr)
            c = QPointF(at.x() + lx * r * 0.35, at.y() + ly * r * 0.3)
            p.setPen(Qt.NoPen)
            p.setBrush(QColor("#141010"))
            if slit:
                w = rr * (0.75 if wide else 0.32)
                p.drawEllipse(c, w, rr * 0.95)
            else:
                p.drawEllipse(c, rr * (0.6 if iris is not None else 1.0), rr * (0.6 if iris is not None else 1.0))
            p.setBrush(QColor("#FFFFFF"))
            p.drawEllipse(QPointF(c.x() - rr * 0.3, c.y() - rr * 0.35), rr * 0.3, rr * 0.3)
        else:
            path = QPainterPath()
            x, y, w = at.x(), at.y(), r * 1.3
            if pose.eyes == "closed":
                path.moveTo(x - w, y)
                path.quadTo(x, y + w * 0.8, x + w, y)
            elif pose.eyes == "happy":
                path.moveTo(x - w, y + w * 0.4)
                path.quadTo(x, y - w, x + w, y + w * 0.4)
            else:  # dizzy
                path.moveTo(x - w * 0.8, y - w * 0.8)
                path.lineTo(x + w * 0.8, y + w * 0.8)
                path.moveTo(x + w * 0.8, y - w * 0.8)
                path.lineTo(x - w * 0.8, y + w * 0.8)
            p.setBrush(Qt.NoBrush)
            p.setPen(pen(stroke, 1.4))
            p.drawPath(path)

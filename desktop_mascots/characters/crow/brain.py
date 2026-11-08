"""Crow: pose and behaviour (struts, pecks, caws, tilts its head, flies, hoards shiny coins)."""
from __future__ import annotations

import math
from dataclasses import dataclass

from ...engine import BasePose, Bounds, Brain, bump, clamp, ease, keyframes, lerp, lerp2  # noqa: F401

REST_HEAD = (16.0, -54.0)
CAW_TIME = 1.3
SHINY_TIME = 4.6
TILTS = (-22.0, 8.0, -30.0, 14.0, -12.0, 4.0)  # jerky, bird-like head angles


@dataclass
class CrowPose(BasePose):
    head: tuple[float, float] = REST_HEAD   # head centre, relative to the feet (facing +x)
    head_angle: float = 0.0     # degrees, positive tips the beak down
    beak_open: float = 0.0      # 0..1
    wing: float = 0.0           # -0.3 (down-stroke) .. 0 folded .. 1 raised
    leg_front: float = 4.0      # leg angles, degrees, positive = forward
    leg_back: float = -4.0
    tail: float = 0.0           # tail angle offset, degrees (positive = up)
    coin: str = "none"          # none | ground | beak
    glint: float = 0.0          # 0..1 sparkle on the coin


class CrowBrain(Brain):
    pose_cls = CrowPose
    STEAL_LINES = ("Shiny!", "Mine.", "CAW!")
    RETURN_LINES = ("Not shiny enough.", "Kraa. Fine.")
    WALK_SPEED = 65.0
    JUMP_VY = -560.0
    JUMP_VX = 160.0
    AIRBORNE = ("air", "dragged", "fly")
    LINES = ("CAW!", "CAW! CAW!", "Kraa!", "Caw caw?", "Shiny?")
    CHOICES = (
        (28, "walk", (2.0, 5.0)),
        (14, "peck", (2.0, 4.0)),
        (9, "caw", (CAW_TIME, CAW_TIME)),
        (8, "shiny", (SHINY_TIME, SHINY_TIME)),
        (11, "fly", (1.0, 1.0)),          # real duration is set on take-off
        (10, "crouch", (0.12, 0.12)),
        (8, "tilt", (1.5, 3.0)),
        (12, "idle", (1.5, 3.5)),
    )

    def poke(self) -> None:
        """A poked crow caws back at you."""
        if not self.airborne and self.state not in ("crouch", "land", "dizzy"):
            self.go("caw", CAW_TIME)
        else:
            self.say(self.rng.choice(self.LINES))

    def _jerky_tilt(self, period: float, scale: float = 1.0) -> float:
        return TILTS[int(self.clock / period) % len(TILTS)] * scale

    # ----------------------------------------------------------- shared-state poses
    def pose_rest(self, p: CrowPose) -> None:
        p.bob = math.sin(self.clock * 2.0) * 0.4
        p.head_angle = self._jerky_tilt(1.3, 0.4)    # birds look around in little jumps
        p.tail = 3 * math.sin(self.clock * 1.3)

    def pose_walk(self, p: CrowPose, t: float) -> None:
        ph = t * 9.0
        s = math.sin(ph)
        p.leg_front, p.leg_back = 24 * s, -24 * s
        bob = max(0.0, math.sin(ph * 2))                # head thrusts forward each step
        p.head = (REST_HEAD[0] + 4 * bob - 1, REST_HEAD[1] + 1.5 * bob)
        p.tail = 4 * math.sin(ph * 2)
        p.lean = 3

    def pose_crouch(self, p: CrowPose, k: float) -> None:
        self.pose_rest(p)
        p.wing = 0.3 * k
        p.tail = 10 * k

    def pose_jump(self, p: CrowPose) -> None:
        p.wing = 0.5 + 0.5 * math.sin(self.clock * 30)
        p.leg_front, p.leg_back = -30.0, -40.0
        p.tail = -6

    def pose_fall(self, p: CrowPose) -> None:
        c = self.clock
        p.wing = 0.35 + 0.65 * math.sin(c * 30)
        p.beak_open = 0.6 + 0.4 * math.sin(c * 9)
        p.leg_front, p.leg_back = 15 * math.sin(c * 14), -15 * math.sin(c * 14)
        p.tail = 10 * math.sin(c * 11)

    def pose_land(self, p: CrowPose, bend: float) -> None:
        self.pose_rest(p)
        p.wing = 0.7 * bend
        p.tail = 14 * bend

    def pose_dizzy(self, p: CrowPose) -> None:
        c = self.clock
        p.head = (REST_HEAD[0] + 2.5 * math.cos(c * 4), REST_HEAD[1] + 2 * math.sin(c * 4))
        p.head_angle = 14 * math.sin(c * 4)
        p.wing = 0.08

    def pose_dragged(self, p: CrowPose) -> None:
        c = self.clock
        p.wing = 0.35 + 0.65 * math.sin(c * 26)
        p.head_angle = -15
        p.beak_open = abs(math.sin(c * 5))
        p.leg_front, p.leg_back = 20 * math.sin(c * 9), -20 * math.sin(c * 9)
        p.tail = 12 * math.sin(c * 13)

    # ----------------------------------------------------------- crow states
    def _peck(self, dt: float, b: Bounds, p: CrowPose) -> None:
        self.pose_rest(p)
        k = ease(self.t / 0.3) * ease((self.duration - self.t) / 0.3)
        peck = max(0.0, math.sin(self.clock * 10)) ** 3
        p.head = lerp2(p.head, (27, -25 + 4 * peck), k)   # tips the whole body, not just the neck
        p.head_angle = lerp(p.head_angle, 62, k)
        p.lean = 10 * k
        p.tail = lerp(p.tail, 12, k)
        if self.done:
            self.go_idle()

    def _caw(self, dt: float, b: Bounds, p: CrowPose) -> None:
        self.pose_rest(p)
        if self.entered:
            self.say(self.rng.choice(("CAW!", "CAW! CAW!", "Kraa!")), 1.5)
        k = ease(self.t / 0.15) * ease((self.duration - self.t) / 0.2)
        caw = bump(self.t, 0.3, 0.35) + bump(self.t, 0.85, 0.35)
        p.head = lerp2(p.head, (21, -55), k)
        p.head_angle = -14 * k - 6 * caw
        p.beak_open = caw
        p.squash = 1 - 0.05 * caw          # the whole body pumps with each caw
        p.wing = 0.12 * caw
        p.tail = 4 + 10 * caw
        if self.done:
            self.go_idle()

    def _tilt(self, dt: float, b: Bounds, p: CrowPose) -> None:
        """Curious: snaps its head between angles while its eye stays on the cursor."""
        self.pose_rest(p)
        p.head_angle = self._jerky_tilt(0.45)
        p.head = (REST_HEAD[0] + 1, REST_HEAD[1] - 1)
        if self.done:
            self.go_idle()

    def _shiny(self, dt: float, b: Bounds, p: CrowPose) -> None:
        """Spot a coin, pick it up, admire it, tuck it under the wing."""
        self.pose_rest(p)
        t = self.t
        head, angle = keyframes(t, (
            (0.0, REST_HEAD, 0.0),
            (0.5, (24.0, -40.0), 35.0),       # notice it
            (0.8, (27.0, -25.0), 62.0),       # pick it up
            (1.0, (27.0, -25.0), 62.0),
            (1.35, (19.0, -57.0), -10.0),     # hold it up
            (3.4, (19.0, -57.0), -10.0),
            (3.9, (-2.0, -47.0), 140.0),      # tuck it under the wing
            (4.15, (-2.0, -47.0), 140.0),
            (SHINY_TIME, REST_HEAD, 0.0),
        ))
        p.head = head
        p.head_angle = angle
        p.lean = 10 * clamp((head[1] + 40) / 15, 0.0, 1.0)  # lean in as the head goes down
        if 1.35 < t < 3.4:
            p.head_angle += self._jerky_tilt(0.5, 0.9)
            p.glint = max(bump(t, 1.7, 0.4), bump(t, 2.7, 0.4))
            if 0.6 < (t - 1.35) % 1.0 < 0.9:
                p.eyes = "happy"
        if 3.4 < t < 4.15:
            p.wing = 0.15
        p.coin = "ground" if t < 0.9 else "beak" if t < 4.0 else "none"
        if t - dt < 1.4 <= t:
            self.say(self.rng.choice(("Shiny!", "Mine.", "Ooh.")), 1.8)
        if self.done:
            self.go_idle()

    def pose_flight(self, p: CrowPose, prog: float) -> None:
        descending = prog > 0.5
        # strong flapping on the way up, slower glide-flaps coming down
        p.wing = 0.35 + 0.65 * math.sin(self.clock * (16 if descending else 24))
        p.lean = 12 if not descending else 4
        landing = ease((prog - 0.8) / 0.2)
        p.leg_front = lerp(-55.0, 10.0, landing)       # legs tucked back, then reach for the ground
        p.leg_back = lerp(-65.0, -5.0, landing)
        p.tail = -4 + 10 * landing
        p.head = (REST_HEAD[0] + 2, REST_HEAD[1] + 2)

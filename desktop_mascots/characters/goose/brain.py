"""Goose: pose and behaviour (waddles, grazes, honks, preens, charges at the cursor)."""
from __future__ import annotations

import math
from dataclasses import dataclass

from ...engine import BasePose, Bounds, Brain, bump, ease, lerp, lerp2

REST_HEAD = (19.0, -78.0)
CHASE_SPEED = 150.0
HONK_TIME = 1.1


@dataclass
class GoosePose(BasePose):
    head: tuple[float, float] = REST_HEAD   # head centre, relative to the feet (facing +x)
    head_angle: float = 0.0     # degrees, positive tips the beak down
    neck_bend: float = 0.0      # extra bulge of the neck curve
    beak_open: float = 0.0      # 0..1
    wing: float = 0.0           # 0 folded .. 1 raised
    leg_front: float = 4.0      # leg angles, degrees, positive = forward
    leg_back: float = -4.0
    tail: float = 0.0           # wag, -1..1


class GooseBrain(Brain):
    pose_cls = GoosePose
    WALK_SPEED = 55.0
    JUMP_VY = -620.0
    LINES = ("HONK!", "Honk?", "HONK HONK!", "Hssss!")
    CHOICES = (
        (38, "walk", (3.0, 7.0)),
        (18, "graze", (2.5, 5.0)),
        (10, "honk", (HONK_TIME, HONK_TIME)),
        (9, "preen", (2.0, 3.5)),
        (8, "chase", (2.5, 4.5)),
        (5, "crouch", (0.16, 0.16)),
        (14, "idle", (1.5, 4.0)),
    )

    def poke(self) -> None:
        """Geese don't hop when poked. They honk back."""
        if not self.airborne and self.state not in ("crouch", "land", "dizzy"):
            self.go("honk", HONK_TIME)
        else:
            self.say(self.rng.choice(self.LINES))

    # ----------------------------------------------------------- shared-state poses
    def pose_rest(self, p: GoosePose) -> None:
        s = math.sin(self.clock * 1.8)
        p.bob = s * 0.5
        p.head = (REST_HEAD[0] + 0.8 * math.sin(self.clock * 0.9), REST_HEAD[1] + 0.8 * s)
        p.tail = math.sin(self.clock * 1.1) * 0.5

    def pose_walk(self, p: GoosePose, t: float) -> None:
        ph = t * 8.0
        s = math.sin(ph)
        p.leg_front, p.leg_back = 22 * s, -22 * s
        p.lean = 2.5 * math.sin(ph * 2)                      # rocking waddle
        p.head = (21 + 2.5 * math.cos(ph * 2), -77 + 1.2 * math.sin(ph * 2))
        p.tail = s

    def pose_crouch(self, p: GoosePose, k: float) -> None:
        self.pose_rest(p)
        p.wing = 0.35 * k
        p.head = lerp2(p.head, (22, -72), k)

    def pose_jump(self, p: GoosePose) -> None:
        p.wing = 0.65 + 0.35 * math.sin(self.clock * 30)
        p.head = (24, -80)
        p.beak_open = 0.4
        p.leg_front, p.leg_back = -20.0, -35.0

    def pose_fall(self, p: GoosePose) -> None:
        c = self.clock
        p.wing = 0.6 + 0.4 * math.sin(c * 34)
        p.head = (25, -75)
        p.beak_open = 0.7 + 0.3 * math.sin(c * 9)
        p.leg_front, p.leg_back = 12 * math.sin(c * 15), -12 * math.sin(c * 15)

    def pose_land(self, p: GoosePose, bend: float) -> None:
        self.pose_rest(p)
        p.wing = 0.6 * bend
        p.head = lerp2(p.head, (24, -70), bend)

    def pose_dizzy(self, p: GoosePose) -> None:
        c = self.clock
        p.head = (REST_HEAD[0] + 3 * math.cos(c * 4), REST_HEAD[1] + 2 * math.sin(c * 4))
        p.head_angle = 12 * math.sin(c * 4)
        p.beak_open = 0.25
        p.wing = 0.1

    def pose_dragged(self, p: GoosePose) -> None:
        c = self.clock
        p.wing = 0.5 + 0.5 * abs(math.sin(c * 25))
        p.head = (16, -84)
        p.head_angle = -15
        p.beak_open = abs(math.sin(c * 6))
        p.leg_front, p.leg_back = 25 * math.sin(c * 10), -25 * math.sin(c * 10)
        p.tail = math.sin(c * 12)

    # ----------------------------------------------------------- goose states
    def _graze(self, dt: float, b: Bounds, p: GoosePose) -> None:
        self.pose_rest(p)
        k = ease(self.t / 0.4) * ease((self.duration - self.t) / 0.4)
        peck = max(0.0, math.sin(self.clock * 9)) ** 4
        p.head = lerp2(p.head, (40, -12 + 5 * peck), k)
        p.head_angle = 75 * k
        p.beak_open = 0.35 * peck * k
        p.tail = math.sin(self.clock * 3) * 0.6
        if self.done:
            self.go_idle()

    def _honk(self, dt: float, b: Bounds, p: GoosePose) -> None:
        self.pose_rest(p)
        if self.entered:
            self.say(self.rng.choice(("HONK!", "HONK HONK!")), 1.4)
        k = ease(self.t / 0.15) * ease((self.duration - self.t) / 0.2)
        p.head = lerp2(p.head, (32, -70), k)
        p.head_angle = -18 * k
        p.neck_bend = -4 * k
        p.beak_open = bump(self.t, 0.3, 0.35) + bump(self.t, 0.75, 0.35)
        p.wing = 0.25 * k
        p.tail = math.sin(self.clock * 14) * k
        if self.done:
            self.go_idle()

    def _preen(self, dt: float, b: Bounds, p: GoosePose) -> None:
        self.pose_rest(p)
        k = ease(self.t / 0.35) * ease((self.duration - self.t) / 0.35)
        c = self.clock
        p.head = lerp2(p.head, (-6 + 3 * math.sin(c * 14), -56 + 2 * math.cos(c * 14)), k)
        p.head_angle = 150 * k
        p.wing = k * (0.12 + 0.05 * math.sin(c * 14))
        if k > 0.8:
            p.eyes = "closed"
        if self.done:
            self.go_idle()

    def _chase(self, dt: float, b: Bounds, p: GoosePose) -> None:
        dx = self.cursor[0] if self.cursor else 0.0
        if self.paused or self.done or self.cursor is None:
            self.pose_rest(p)
            self.go_idle()
            return
        if abs(dx) < 45:            # caught up with the cursor: tell it off
            self.pose_rest(p)
            self.go("honk", HONK_TIME)
            return
        if self.entered:
            self.say("Hssss!", 1.0)
        self.facing = 1 if dx > 0 else -1
        self.step_x(b, self.facing * CHASE_SPEED * dt)
        self.pose_walk(p, self.t * 1.9)
        c = self.clock
        p.head = (31 + 1.5 * math.sin(c * 30), -67)   # neck stretched out low, menacing
        p.head_angle = 6
        p.neck_bend = 3
        p.beak_open = 0.35 + 0.25 * math.sin(c * 20)
        p.wing = 0.35 + 0.08 * math.sin(c * 18)
        p.lean = 6

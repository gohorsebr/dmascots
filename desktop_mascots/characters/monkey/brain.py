"""Monkey: pose and behaviour (walks, scratches its head, eats bananas, hops)."""
from __future__ import annotations

import math
from dataclasses import dataclass

from ...engine import BasePose, Bounds, Brain, bump, ease, lerp

EAT_TIME = 5.2
BITES = (1.2, 2.3, 3.4)


@dataclass
class MonkeyPose(BasePose):
    # limb angles: degrees from hanging straight down, positive toward the facing side;
    # each limb is (upper_segment, lower_relative)
    head_tilt: float = 0.0
    arm_front: tuple[float, float] = (12.0, 10.0)
    arm_back: tuple[float, float] = (-12.0, -10.0)
    leg_front: tuple[float, float] = (3.0, 0.0)
    leg_back: tuple[float, float] = (-3.0, 0.0)
    tail: float = 0.0           # sway, -1..1
    mouth: str = "smile"        # smile | big | o | chew
    banana: int = -1            # -1 = no banana, otherwise bites taken (0..3)


class MonkeyBrain(Brain):
    pose_cls = MonkeyPose
    WALK_SPEED = 75.0
    LINES = ("Ook!", "Ook ook!", "Eek!", "Banana?", "Hi!")
    CHOICES = (
        (45, "walk", (2.5, 7.0)),
        (14, "scratch", (1.8, 3.2)),
        (10, "eat", (EAT_TIME, EAT_TIME)),
        (9, "crouch", (0.14, 0.14)),
        (22, "idle", (1.5, 4.0)),
    )

    # ----------------------------------------------------------- shared-state poses
    def pose_rest(self, p: MonkeyPose) -> None:
        s = math.sin(self.clock * 2.2)
        p.bob = s * 0.7
        p.tail = math.sin(self.clock * 1.4)
        p.arm_front = (12 + 2 * s, 10.0)
        p.arm_back = (-12 - 2 * s, -10.0)

    def pose_walk(self, p: MonkeyPose, t: float) -> None:
        ph = t * 9.0
        s, c = math.sin(ph), math.cos(ph)
        p.leg_front = (24 * s, -28 * max(0.0, c))
        p.leg_back = (-24 * s, -28 * max(0.0, -c))
        p.arm_front = (12 - 22 * s, 12.0)
        p.arm_back = (-12 + 22 * s, 12.0)
        p.lean = 4.0
        p.tail = math.sin(self.clock * 5) * 0.8

    def pose_crouch(self, p: MonkeyPose, k: float) -> None:
        self.pose_rest(p)
        p.leg_front = (28 * k, -56 * k)
        p.leg_back = (-28 * k, 56 * k)
        p.arm_front = (12 + 25 * k, 10.0)
        p.arm_back = (-12 - 25 * k, -10.0)

    def pose_jump(self, p: MonkeyPose) -> None:
        p.tail = math.sin(self.clock * 8)
        p.arm_front, p.arm_back = (150.0, 15.0), (-150.0, -15.0)
        p.leg_front, p.leg_back = (35.0, -70.0), (-35.0, 70.0)
        p.mouth = "big"

    def pose_fall(self, p: MonkeyPose) -> None:
        c = self.clock
        w = math.sin(c * 22)
        p.tail = math.sin(c * 8)
        p.arm_front = (135 + 30 * w, 15.0)
        p.arm_back = (-135 + 30 * w, -15.0)
        p.leg_front = (20 * math.sin(c * 16), -25.0)
        p.leg_back = (-20 * math.sin(c * 16 + 1.3), 25.0)
        p.mouth = "o"

    def pose_land(self, p: MonkeyPose, bend: float) -> None:
        self.pose_rest(p)
        p.leg_front = (30 * bend, -60 * bend)
        p.leg_back = (-30 * bend, 60 * bend)
        p.arm_front = (12 + 50 * bend, 10.0)
        p.arm_back = (-12 - 50 * bend, -10.0)

    def pose_dizzy(self, p: MonkeyPose) -> None:
        p.mouth = "o"
        p.head_tilt = 7 * math.sin(self.clock * 3.2 + 1.2)

    def pose_dragged(self, p: MonkeyPose) -> None:
        c = self.clock
        p.arm_front = (28 + 8 * math.sin(c * 5), 12.0)
        p.arm_back = (-28 - 8 * math.sin(c * 5 + 0.6), -12.0)
        p.leg_front = (12 * math.sin(c * 7), -18.0)
        p.leg_back = (-12 * math.sin(c * 7 + 1.6), 18.0)
        p.mouth = "o"
        p.tail = math.sin(c * 3)

    # ----------------------------------------------------------- monkey states
    def _scratch(self, dt: float, b: Bounds, p: MonkeyPose) -> None:
        self.pose_rest(p)
        k = ease(self.t / 0.3) * ease((self.duration - self.t) / 0.3)
        up = (-162.0, -28.0 + 14 * math.sin(self.clock * 28))
        p.arm_back = (lerp(p.arm_back[0], up[0], k), lerp(p.arm_back[1], up[1], k))
        p.head_tilt = -8 * k
        if 0.35 < self.t / self.duration < 0.8:
            p.eyes = "happy"
        if self.done:
            self.go_idle()

    def _eat(self, dt: float, b: Bounds, p: MonkeyPose) -> None:
        self.pose_rest(p)
        t = self.t
        k = ease(t / 0.5) * ease((EAT_TIME - t) / 0.5)
        bring = max(bump(t, bt, 0.45) for bt in BITES)
        p.arm_front = (lerp(p.arm_front[0], 30.0, k), lerp(p.arm_front[1], 130.0, k) + 50 * bring)
        bites = sum(t > bt for bt in BITES)
        finished = t >= 4.2
        p.banana = -1 if finished else bites
        p.head_tilt = 4 * bring
        if bring > 0.35:
            p.mouth = "o"
        elif finished:
            p.mouth, p.eyes = "big", "happy"
        elif bites:
            p.mouth = "chew" if math.sin(self.clock * 16) > 0 else "smile"
        if t >= EAT_TIME:
            self.go_idle()

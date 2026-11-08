"""Parrot: waddles, dances, whistles, preens, flies, and repeats what you've said to it."""
from __future__ import annotations

import math
from dataclasses import dataclass

from ...engine import BasePose, Bounds, Brain, bump, ease, lerp, lerp2

REST_HEAD = (5.0, -56.0)
WHISTLE_TIME = 1.8
PHRASES = ("Hello!", "Pretty bird!", "Who's a good bird?", "Hehehe.", "Cracker?", "Hello? Hello!")


@dataclass
class ParrotPose(BasePose):
    head: tuple[float, float] = REST_HEAD
    head_angle: float = 0.0
    beak_open: float = 0.0
    wing: float = 0.0           # -0.3 (down-stroke) .. 0 folded .. 1 raised
    leg_front: float = 4.0
    leg_back: float = -4.0
    tail: float = 0.0           # degrees, positive = up
    notes: float | None = None  # music notes floating up (dancing / whistling)
    fluff: float = 0.0          # 0..1 feathers puffed up


class ParrotBrain(Brain):
    pose_cls = ParrotPose
    WALK_SPEED = 32.0
    JUMP_VY = -520.0
    JUMP_VX = 120.0
    AIRBORNE = ("air", "dragged", "fly")
    FLY_SPEED = 300.0
    LINES = ("SQUAWK!",) + PHRASES
    CHOICES = (
        (18, "walk", (2.0, 4.0)),
        (10, "dance", (3.0, 6.0)),
        (13, "mimic", (2.0, 2.0)),        # real duration depends on what it says
        (8, "whistle", (WHISTLE_TIME, WHISTLE_TIME)),
        (8, "preen", (2.0, 3.5)),
        (10, "fly", (1.0, 1.0)),
        (6, "crouch", (0.14, 0.14)),
        (14, "idle", (1.5, 4.0)),
    )

    def poke(self) -> None:
        """Poke a parrot and it talks back -- often with something you told it."""
        if not self.airborne and self.state not in ("crouch", "land", "dizzy"):
            self.go("mimic", 2.0)
        else:
            self.say("SQUAWK!")

    def _jerky(self, period: float, amount: float) -> float:
        return amount * (-1, 0.4, -0.6, 1, -0.2, 0.7)[int(self.clock / period) % 6]

    # ----------------------------------------------------------- shared-state poses
    def pose_rest(self, p: ParrotPose) -> None:
        p.bob = math.sin(self.clock * 1.8) * 0.4
        p.head_angle = self._jerky(1.1, 10)
        p.tail = 2 * math.sin(self.clock * 1.2)

    def pose_walk(self, p: ParrotPose, t: float) -> None:
        ph = t * 8.0
        s = math.sin(ph)
        p.leg_front, p.leg_back = 18 * s, -18 * s
        p.lean = 6 * s                    # side-to-side waddle
        p.bob = -1.2 * abs(s)
        p.tail = 4 * s

    def pose_crouch(self, p: ParrotPose, k: float) -> None:
        self.pose_rest(p)
        p.wing = 0.25 * k

    def pose_jump(self, p: ParrotPose) -> None:
        p.wing = 0.5 + 0.5 * math.sin(self.clock * 28)
        p.leg_front, p.leg_back = -25.0, -35.0

    def pose_fall(self, p: ParrotPose) -> None:
        c = self.clock
        p.wing = 0.35 + 0.65 * math.sin(c * 30)
        p.beak_open = 0.7
        p.fluff = 0.6
        p.tail = 10 * math.sin(c * 11)

    def pose_land(self, p: ParrotPose, bend: float) -> None:
        self.pose_rest(p)
        p.wing = 0.6 * bend
        p.fluff = 0.4 * bend

    def pose_dizzy(self, p: ParrotPose) -> None:
        c = self.clock
        p.head = (REST_HEAD[0] + 2.5 * math.cos(c * 4), REST_HEAD[1] + 2 * math.sin(c * 4))
        p.head_angle = 14 * math.sin(c * 4)
        p.fluff = 0.3

    def pose_dragged(self, p: ParrotPose) -> None:
        c = self.clock
        p.wing = 0.35 + 0.65 * math.sin(c * 26)
        p.beak_open = abs(math.sin(c * 5))
        p.fluff = 0.8
        p.leg_front, p.leg_back = 20 * math.sin(c * 9), -20 * math.sin(c * 9)
        p.tail = 12 * math.sin(c * 13)

    def pose_flight(self, p: ParrotPose, prog: float) -> None:
        descending = prog > 0.5
        p.wing = 0.35 + 0.65 * math.sin(self.clock * (16 if descending else 24))
        p.lean = 35 if not descending else 22    # body stretched out level
        landing = ease((prog - 0.8) / 0.2)
        p.leg_front = lerp(-60.0, 8.0, landing)
        p.leg_back = lerp(-70.0, -6.0, landing)
        p.tail = -14 + 14 * landing
        p.head = (REST_HEAD[0] + 3, REST_HEAD[1] + 3)

    # ----------------------------------------------------------- parrot states
    def _dance(self, dt: float, b: Bounds, p: ParrotPose) -> None:
        """Head-bobbing to a beat only it can hear."""
        self.pose_rest(p)
        beat = abs(math.sin(math.pi * self.t / 0.5))
        p.head = (REST_HEAD[0] + 2 * beat, REST_HEAD[1] + 4 * beat)
        p.head_angle = 15 * beat
        p.bob = 2 * beat
        p.lean = 6 * (1 if int(self.t / 1.0) % 2 else -1) * ease(beat)
        p.wing = 0.15 * beat
        p.leg_front, p.leg_back = 6 * beat, -6 * beat
        p.notes = self.clock
        if self.entered and self.rng.random() < 0.4:
            self.say("♪ ♫", 1.4)
        if self.done:
            self.go_idle()

    def _mimic(self, dt: float, b: Bounds, p: ParrotPose) -> None:
        """Say something: usually a message it was sent, otherwise a stock phrase."""
        self.pose_rest(p)
        if self.entered:
            if self.heard and self.rng.random() < 0.7:
                text = self.rng.choice(self.heard)
            else:
                text = self.rng.choice(PHRASES)
            self.duration = min(5.0, max(1.6, 0.6 + 0.07 * len(text)))
            self.say(text, self.duration + 0.4)
        talking = self.t < self.duration - 0.3
        p.beak_open = max(0.0, math.sin(self.clock * 18)) * 0.7 if talking else 0.0
        p.head_angle = self._jerky(0.35, 18)
        p.head = (REST_HEAD[0] + 2, REST_HEAD[1])
        if self.done:
            self.go_idle()

    def _whistle(self, dt: float, b: Bounds, p: ParrotPose) -> None:
        self.pose_rest(p)
        if self.entered:
            self.say(self.rng.choice(("♪ Fweet fweeoo! ♪", "♪ Wheet-wheeoo ♪", "♪ Tweedle-dee ♪")), 1.8)
        k = ease(self.t / 0.2) * ease((self.duration - self.t) / 0.3)
        p.head = lerp2(REST_HEAD, (REST_HEAD[0] - 1, REST_HEAD[1] - 3), k)
        p.head_angle = -20 * k
        p.beak_open = 0.25 * k + 0.1 * math.sin(self.clock * 20) * k
        p.notes = self.clock
        p.fluff = 0.3 * k
        if self.done:
            self.go_idle()

    def _preen(self, dt: float, b: Bounds, p: ParrotPose) -> None:
        self.pose_rest(p)
        k = ease(self.t / 0.35) * ease((self.duration - self.t) / 0.35)
        c = self.clock
        p.head = lerp2(REST_HEAD, (-5 + 2 * math.sin(c * 14), -42 + 2 * math.cos(c * 14)), k)
        p.head_angle = 125 * k
        p.wing = 0.12 * k
        p.fluff = 0.5 * k
        if k > 0.8:
            p.eyes = "closed"
        if self.done:
            self.go_idle()
